use std::io::{self, Read};
use std::time::Duration;
use serialport;
use cobs;
use prost::Message;

// O compilador de Protobuf do Rust irá gerar essa struct automaticamente para você,
// mas ela se parecerá com isso no código:
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlocoDados {
    #[prost(uint32, tag = "1")]
    pub id_bloco: u32,
    #[prost(uint32, tag = "2")]
    pub timestamp: u32,
    #[prost(float, repeated, tag = "3")]
    pub leituras: ::prost::alloc::vec::Vec<f32>,
}

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
                        
                        // 2. Decodifica o COBS "in-place" (na própria memória do vetor)
                        // Isso é extremamente rápido em Rust
                        if let Ok(dados_protobuf) = cobs::decode_vec(&pacote_acumulado) {
                            
                            // 3. Deserializa o Protobuf usando a crate Prost
                            match BlocoDados::decode(&dados_protobuf[..]) {
                                Ok(bloco) => {
                                    // SUCESSO! Seus dados prontos para uso
                                    println!(
                                        "Bloco #{} recebido. Timestamp: {}. Floats: {}", 
                                        bloco.id_bloco, bloco.timestamp, bloco.leituras.len()
                                    );

                                    // 1. Pega os primeiros 10 elementos de forma segura 
                                    // (se o vetor tiver menos de 10, ele pega apenas o que existir)
                                    let primeiros_10 = bloco.leituras.iter().take(10);

                                    // 2. Printa os valores na mesma linha
                                    print!("Primeiras 10 leituras: ");
                                    for (i, valor) in primeiros_10.enumerate() {
                                        print!("[{}: {:.4}] ", i, valor); // {:.4} limita o float a 4 casas decimais
                                    }
                                    println!(); // Quebra a linha no final
                                    
                                    // Exemplo de acesso ao primeiro float:
                                    // println!("Primeira leitura: {}", bloco.leituras[0]);
                                }
                                Err(e) => eprintln!("Erro ao decodificar Protobuf: {:?}", e),
                            }
                        } else {
                            eprintln!("Erro no checksum/padrão do COBS.");
                        }
                        
                        pacote_acumulado.clear(); // Limpa o buffer para o próximo bloco
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