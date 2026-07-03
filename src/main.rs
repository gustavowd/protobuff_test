use std::io::{self, Read};
use std::time::Duration;
use prost::Message;
use crc32fast::Hasher;

// ==========================================
// Injeção Correta do Código Gerado
// ==========================================
pub mod proto {
    // O prost-build por padrão gera o arquivo com o nome padrão do escopo (geralmente vazio se sem pacote, caindo em _.rs)
    // Para garantir que o arquivo correto seja puxado, usamos a macro apontando para o arquivo gerado
    include!(concat!(env!("OUT_DIR"), "/_.rs")); 
}

// O Prost converte os nomes para o padrão Snake Case do Rust!
// MensagemSerial -> mensagem_serial
// BlocoDados -> bloco_dados
// StatusSistema -> status_sistema
use proto::{mensagem_serial, MensagemSerial};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configura e abre a porta serial
    let mut porta = serialport::new("/dev/ttyACM0", 115_200)
        .timeout(Duration::from_millis(10))
        .open()?;

    println!("Conectado à porta serial. Aguardando blocos de 1024 floats...");

    let mut buffer_leitura = [0u8; 1];
    let mut pacote_acumulado = Vec::with_capacity(5000); // Buffer com folga para os ~4KB

    loop {
        // Lê byte a byte da serial
        match porta.read_exact(&mut buffer_leitura) {
            Ok(_) => {
                let byte = buffer_leitura[0];

                if byte == 0x00 { // Encontrou o fim do pacote!
                    if !pacote_acumulado.is_empty() {
                        
                        // 2. Decodifica o COBS in-place
                        if let Ok(dados_protobuf_crc) = cobs::decode_vec(&pacote_acumulado) {

                            let len_total = dados_protobuf_crc.len();

                            // 2.5 Validação de Integridade via CRC32 (Little Endian)
                            // O pacote precisa ter pelo menos mais que 4 bytes (que é o tamanho do CRC)
                            if len_total > 4 {
                                let tamanho_protobuf = len_total - 4;

                                // Separa os dados puros do protobuf e a cauda com os 4 bytes do CRC
                                let dados_protobuf = &dados_protobuf_crc[0..tamanho_protobuf];
                                let bytes_crc = &dados_protobuf_crc[tamanho_protobuf..len_total];

                                // Reconstrói o u32 a partir dos 4 bytes em LITTLE ENDIAN
                                let crc_recebido = u32::from_le_bytes([
                                    bytes_crc[0], bytes_crc[1], bytes_crc[2], bytes_crc[3]
                                ]);

                                // Calcula o CRC32 local sobre a parte do Protobuf
                                let mut hasher = Hasher::new();
                                hasher.update(dados_protobuf);
                                let crc_calculado = hasher.finalize();

                                // Verifica se o CRC confere
                                if crc_recebido == crc_calculado {
                            
                                    // Deserializa o ENVELOPE principal em vez do BlocoDados diretamente
                                    match MensagemSerial::decode(dados_protobuf) {
                                        Ok(envelope) => {
                                            
                                            // 4. Pattern Matching para extrair o conteúdo de dentro do Oneof
                                            if let Some(conteudo_interno) = envelope.conteudo {
                                                match conteudo_interno {
                                                    // CASO A: A mensagem recebida é um BlocoDados
                                                    mensagem_serial::Conteudo::BlocoDados(bloco) => {
                                                        println!(
                                                            "\n[TELEMETRIA] Bloco #{} recebido. Timestamp: {}. Total de Floats: {}", 
                                                            bloco.id_bloco, bloco.timestamp, bloco.leituras.len()
                                                        );

                                                        // Printa os primeiros 10 elementos de forma segura
                                                        let primeiros_10 = bloco.leituras.iter().take(10);
                                                        print!("Primeiras 10 leituras: ");
                                                        for (i, valor) in primeiros_10.enumerate() {
                                                            print!("[{}: {:.4}] ", i, valor);
                                                        }
                                                        println!();
                                                    }

                                                    // CASO B: A mensagem recebida é o StatusSistema
                                                    mensagem_serial::Conteudo::StatusSistema(status) => {
                                                        println!("\n[DIAGNÓSTICO] Mensagem de status recebida!");
                                                        println!("-> Timestamp: {}", status.timestamp);
                                                        println!("-> Temperatura do chip: {}°C", status.temperatura);
                                                        println!("-> Umidade relativa: {}%", status.umidade);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => eprintln!("Erro ao decodificar o envelope Protobuf: {:?}", e),
                                    }
                                }else{
                                    eprintln!(
                                        "Erro de integridade: CRC recebido ({:#010X}) não confere com o CRC calculado ({:#010X}).", 
                                        crc_recebido, crc_calculado
                                    );
                                }
                            }else{
                                eprintln!("Erro: Pacote decodificado é muito pequeno para conter um CRC válido.");
                            }
                        } else {
                            eprintln!("Erro no checksum/padrão do COBS.");
                        }
                        
                        pacote_acumulado.clear();
                    }
                } else {
                    pacote_acumulado.push(byte);
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                // Timeout normal da serial quando não há dados chegando, apenas continua
                continue;
            }
            Err(e) => {
                eprintln!("Erro crítico na leitura da serial: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}