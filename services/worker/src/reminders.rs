//! O ritmo da entrega de lembretes.
//!
//! # Porque aqui só está o relógio
//!
//! A passagem — reclamar, entregar, contar tentativas — vive em
//! `ocinye_core::modules::calendar::delivery`, junto das outras funções de
//! entrega e onde os testes lhe chegam. Aqui fica o que é do worker: de quanto
//! em quanto tempo perguntar.
//!
//! Um lembrete que só dispara com o separador aberto não é um lembrete. A pessoa
//! que pediu para ser avisada às nove da manhã fechou o portátil às seis da
//! tarde do dia anterior.

use std::time::Duration;

/// Com que frequência se procura o que está por entregar.
///
/// Trinta segundos. Um lembrete institucional não precisa de precisão ao
/// segundo, e sondar de segundo a segundo custaria uma consulta por segundo,
/// para sempre, para não ganhar nada que alguém note.
pub const POLL_INTERVAL: Duration = Duration::from_secs(30);
