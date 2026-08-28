//! Continuidade institucional: um servidor pode desaparecer, a instituição não.
//!
//! # A propriedade
//!
//! > **Infrastructure may be replaced. Institutional state must survive.**
//!
//! Um servidor é uma instância de execução. Não é a fonte de verdade da
//! instituição só porque contém fisicamente o disco onde o PostgreSQL está
//! instalado.
//!
//! # Porque isto é arquitectura, e não um procedimento de quem administra
//!
//! Porque a resposta à pergunta «o que é preciso levar?» não se descobre a
//! olhar para o servidor. Descobre-se a olhar para o que o domínio considera
//! estado autoritativo — e essa é uma decisão do Core, não de quem opera a
//! máquina.
//!
//! Um `pg_dump` salva a base. Não salva os bytes a que ela aponta, nem a chave
//! sem a qual parte das linhas é ilegível. Um backup assim é uma cópia
//! perfeitamente íntegra e completamente inútil, e isso só se descobre no dia
//! do desastre.
//!
//! # O que este módulo **não** faz
//!
//! Não é uma segunda fonte de verdade. Não guarda estado próprio, não tem
//! tabelas e não regista nada: lê o que existe, descreve-o, e compara duas
//! descrições. A memória institucional continua onde sempre esteve.

pub mod classification;
pub mod keys;
pub mod manifest;
pub mod models;
pub mod objects;

pub use classification::{inventario, Activo, Classe};
pub use keys::{legibilidade, viaja_por_canal_proprio, Destino, Legibilidade, Material};
pub use manifest::{comparar, descrever, Divergencia, Manifesto};
pub use models::{por_responder, Pergunta, Resposta};
pub use objects::{conferir, Veredicto};
