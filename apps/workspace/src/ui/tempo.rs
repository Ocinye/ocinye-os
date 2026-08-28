//! Datas ditas em português.
//!
//! # Porque isto existe
//!
//! O `chrono` formata `%B` e `%A` em inglês salvo atrás de uma feature marcada
//! como instável. O Calendário estava a escrever «August de 2026» e «Mon Tue
//! Wed» ao lado de texto português — meia frase em cada língua, na superfície
//! que uma pessoa da instituição vê todos os dias.
//!
//! Doze nomes de mês e sete de dia não justificam uma dependência instável, e
//! muito menos justificam ser corrigidos onde aparecem: uma substituição por
//! componente dá cinco vistas a discordar sobre como se abrevia «Sábado». Isto
//! é uma fonte só, e é dela que o Mês, a Semana, o Dia, o Ano e o relógio da
//! barra passam a beber.
//!
//! # Porque não é configurável
//!
//! Porque a instituição é portuguesa e a interface é em português. O dia em que
//! deixar de ser, isto passa a receber um locale — e nessa altura o sítio onde
//! mexer é um só, que é precisamente a razão de existir.

use chrono::{Datelike, NaiveDate};

/// Os meses, de Janeiro a Dezembro.
const MESES: [&str; 12] = [
    "Janeiro",
    "Fevereiro",
    "Março",
    "Abril",
    "Maio",
    "Junho",
    "Julho",
    "Agosto",
    "Setembro",
    "Outubro",
    "Novembro",
    "Dezembro",
];

/// Os dias da semana, de segunda a domingo — a ordem em que a grelha os mostra.
const DIAS: [&str; 7] = [
    "Segunda-feira",
    "Terça-feira",
    "Quarta-feira",
    "Quinta-feira",
    "Sexta-feira",
    "Sábado",
    "Domingo",
];

/// A abreviatura de cada dia, na mesma ordem.
///
/// Três letras porque a grelha tem sete colunas e o cabeçalho não pode ser mais
/// largo do que a coluna que encima.
const DIAS_CURTOS: [&str; 7] = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];

/// O nome do mês desta data.
#[must_use]
pub fn mes(data: NaiveDate) -> &'static str {
    // `month()` devolve 1..=12 por contrato do `chrono`; o `saturating_sub`
    // existe para que um valor impossível não faça pânico numa página.
    MESES[(data.month() as usize).saturating_sub(1).min(11)]
}

/// O nome do dia da semana desta data.
#[must_use]
pub fn dia_da_semana(data: NaiveDate) -> &'static str {
    DIAS[data.weekday().num_days_from_monday() as usize]
}

/// A abreviatura do dia da semana desta data.
#[must_use]
pub fn dia_da_semana_curto(data: NaiveDate) -> &'static str {
    DIAS_CURTOS[data.weekday().num_days_from_monday() as usize]
}

/// Os sete cabeçalhos da grelha, de segunda a domingo.
#[must_use]
pub const fn cabecalhos_da_semana() -> [&'static str; 7] {
    DIAS_CURTOS
}

/// `Agosto 2026` — o título de um mês.
///
/// Sem «de» no meio: é um rótulo, não uma frase.
#[must_use]
pub fn mes_e_ano(data: NaiveDate) -> String {
    format!("{} {}", mes(data), data.year())
}

/// `26 de Agosto de 2026` — uma data por extenso.
#[must_use]
pub fn data_por_extenso(data: NaiveDate) -> String {
    format!("{} de {} de {}", data.day(), mes(data), data.year())
}

/// `Quarta-feira, 26 de Agosto` — uma data com o dia da semana à frente.
#[must_use]
pub fn dia_por_extenso(data: NaiveDate) -> String {
    format!("{}, {} de {}", dia_da_semana(data), data.day(), mes(data))
}

/// `24 – 30 de Agosto de 2026`, ou com os dois meses quando a semana os
/// atravessa.
#[must_use]
pub fn intervalo_da_semana(inicio: NaiveDate, fim: NaiveDate) -> String {
    if inicio.month() == fim.month() && inicio.year() == fim.year() {
        format!(
            "{} – {} de {} de {}",
            inicio.day(),
            fim.day(),
            mes(fim),
            fim.year()
        )
    } else if inicio.year() == fim.year() {
        format!(
            "{} de {} – {} de {} de {}",
            inicio.day(),
            mes(inicio),
            fim.day(),
            mes(fim),
            fim.year()
        )
    } else {
        format!(
            "{} de {} de {} – {} de {} de {}",
            inicio.day(),
            mes(inicio),
            inicio.year(),
            fim.day(),
            mes(fim),
            fim.year()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).expect("data válida")
    }

    /// Os doze meses estão lá, e na ordem.
    ///
    /// Derivado do calendário e não escrito à mão: uma lista repetida no teste
    /// prova que duas listas coincidem, não que a primeira está certa.
    #[test]
    fn os_doze_meses_dizem_se_em_portugues() {
        for m in 1..=12u32 {
            let nome = mes(dia(2026, m, 1));
            assert!(
                !nome.is_empty() && nome.chars().next().is_some_and(char::is_uppercase),
                "o mês {m} saiu como «{nome}»"
            );
            assert!(
                nome.is_ascii() || nome.contains('ç'),
                "«{nome}» não é português"
            );
        }
        assert_eq!(mes(dia(2026, 1, 1)), "Janeiro");
        assert_eq!(mes(dia(2026, 3, 1)), "Março");
        assert_eq!(mes(dia(2026, 8, 26)), "Agosto");
        assert_eq!(mes(dia(2026, 12, 31)), "Dezembro");
    }

    /// A semana começa à segunda, e os nomes acompanham.
    #[test]
    fn os_dias_seguem_a_ordem_da_grelha() {
        // 24/08/2026 é uma segunda-feira.
        let segunda = dia(2026, 8, 24);
        for (offset, esperado) in DIAS_CURTOS.iter().enumerate() {
            let data = segunda + chrono::Duration::days(offset as i64);
            assert_eq!(
                dia_da_semana_curto(data),
                *esperado,
                "o dia {offset} depois de segunda saiu errado"
            );
        }
        assert_eq!(dia_da_semana(segunda), "Segunda-feira");
        assert_eq!(dia_da_semana(segunda + chrono::Duration::days(5)), "Sábado");
    }

    /// Nenhum rótulo sai com uma palavra inglesa lá dentro.
    ///
    /// # Porque este teste procura o defeito e não a correcção
    ///
    /// O defeito era `August de 2026`: metade formatada pelo `chrono` em inglês,
    /// metade escrita por nós em português. Um teste que só confirmasse
    /// «Agosto 2026» passaria à mesma se alguém acrescentasse uma sexta vista
    /// com `%B`. Este pergunta o que interessa — se sobrou inglês.
    #[test]
    fn nenhum_rotulo_temporal_tem_ingles() {
        const INGLES: [&str; 19] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
            "Mon",
            "Tue",
            "Wed",
            "Thu",
            "Fri",
            "Sat",
            "Sun",
        ];

        let mut rotulos = Vec::new();
        for m in 1..=12u32 {
            let data = dia(2026, m, 15);
            rotulos.push(mes_e_ano(data));
            rotulos.push(data_por_extenso(data));
            rotulos.push(dia_por_extenso(data));
        }
        for offset in 0..7i64 {
            let data = dia(2026, 8, 24) + chrono::Duration::days(offset);
            rotulos.push(dia_da_semana(data).to_owned());
            rotulos.push(dia_da_semana_curto(data).to_owned());
        }
        rotulos.push(intervalo_da_semana(dia(2026, 8, 24), dia(2026, 8, 30)));
        rotulos.push(intervalo_da_semana(dia(2026, 8, 31), dia(2026, 9, 6)));
        rotulos.push(intervalo_da_semana(dia(2026, 12, 28), dia(2027, 1, 3)));

        assert!(
            rotulos.len() >= 40,
            "só {} rótulos examinados: o universo é pequeno de mais para provar alguma coisa",
            rotulos.len()
        );

        for rotulo in &rotulos {
            for palavra in INGLES {
                assert!(!rotulo.contains(palavra), "«{rotulo}» contém «{palavra}»");
            }
        }
    }

    /// A semana que muda de mês, e a que muda de ano, dizem-no.
    #[test]
    fn o_intervalo_da_semana_atravessa_fronteiras() {
        assert_eq!(
            intervalo_da_semana(dia(2026, 8, 24), dia(2026, 8, 30)),
            "24 – 30 de Agosto de 2026"
        );
        assert_eq!(
            intervalo_da_semana(dia(2026, 8, 31), dia(2026, 9, 6)),
            "31 de Agosto – 6 de Setembro de 2026"
        );
        assert_eq!(
            intervalo_da_semana(dia(2026, 12, 28), dia(2027, 1, 3)),
            "28 de Dezembro de 2026 – 3 de Janeiro de 2027"
        );
    }
}

// ── O horário que o editor propõe ───────────────────────────────────────

/// Quantos minutos dura uma actividade nova, se ninguém disser o contrário.
///
/// É conveniência, não autoridade: não é duração mínima, não é regra do Core, e
/// a pessoa altera-a livremente. Está aqui, e num sítio só, porque o mesmo
/// número é usado a propor o fim e a acompanhar uma mudança do início.
pub const DURACAO_PADRAO_MINUTOS: i64 = 30;

/// A que horas começa uma actividade marcada agora.
///
/// # Porquê arredondar
///
/// Porque ninguém marca uma reunião para as 19:07. Arredondar para a meia hora
/// seguinte dá um horário que se aceita sem pensar, e é isso que separa abrir o
/// editor e escrever o título de abrir o editor e tomar quatro decisões.
///
/// # A fronteira exacta
///
/// Às 19:30 em ponto, propõe **20:00**. É deliberado e é a mesma regra: «a
/// próxima meia hora». Propor as 19:30 seria propor um começo que já passou no
/// instante em que a pessoa lê o ecrã — e a regra tem de ser uma só, senão o
/// comportamento no segundo exacto deixa de ser previsível.
#[must_use]
pub fn proximo_meio_periodo(agora: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
    use chrono::Timelike;

    let minutos_do_dia = i64::from(agora.hour()) * 60 + i64::from(agora.minute());
    let seguinte = (minutos_do_dia / DURACAO_PADRAO_MINUTOS + 1) * DURACAO_PADRAO_MINUTOS;

    // A meia-noite do dia seguinte é `dia + 1` às 00:00, e não «24:00» — que não
    // existe. Uma actividade proposta às 23:50 começa amanhã, e é isso que se
    // escreve.
    agora
        .date()
        .and_hms_opt(0, 0, 0)
        .unwrap_or(agora)
        .checked_add_signed(chrono::Duration::minutes(seguinte))
        .unwrap_or(agora)
}

/// O início e o fim que o editor propõe para uma actividade marcada agora.
#[must_use]
pub fn horario_proposto(
    agora: chrono::NaiveDateTime,
) -> (chrono::NaiveDateTime, chrono::NaiveDateTime) {
    let inicio = proximo_meio_periodo(agora);
    (
        inicio,
        inicio + chrono::Duration::minutes(DURACAO_PADRAO_MINUTOS),
    )
}

/// `2026-08-26T19:30` — o formato que um `datetime-local` aceita.
#[must_use]
pub fn para_campo(instante: chrono::NaiveDateTime) -> String {
    instante.format("%Y-%m-%dT%H:%M").to_string()
}

#[cfg(test)]
mod horario {
    use super::*;
    use chrono::Timelike;

    fn quando(a: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(a, m, d)
            .expect("data")
            .and_hms_opt(h, min, 0)
            .expect("hora")
    }

    /// Os casos que a regra tem de acertar, escritos um a um.
    #[test]
    fn a_proposta_arredonda_para_a_proxima_meia_hora() {
        for (agora, esperado) in [
            ((19, 7), (19, 30)),
            ((19, 29), (19, 30)),
            ((19, 31), (20, 0)),
            ((19, 59), (20, 0)),
            ((0, 1), (0, 30)),
        ] {
            let obtido = proximo_meio_periodo(quando(2026, 8, 26, agora.0, agora.1));
            assert_eq!(
                (obtido.hour(), obtido.minute()),
                esperado,
                "às {:02}:{:02} devia propor {:02}:{:02}",
                agora.0,
                agora.1,
                esperado.0,
                esperado.1
            );
        }
    }

    /// A fronteira exacta, dita explicitamente.
    ///
    /// Às 19:30 em ponto propõe 20:00, e não 19:30. A escolha está testada
    /// porque é a que muda conforme quem a escreve.
    #[test]
    fn a_meia_hora_exacta_propoe_a_seguinte() {
        let obtido = proximo_meio_periodo(quando(2026, 8, 26, 19, 30));
        assert_eq!((obtido.hour(), obtido.minute()), (20, 0));

        let obtido = proximo_meio_periodo(quando(2026, 8, 26, 0, 0));
        assert_eq!((obtido.hour(), obtido.minute()), (0, 30));
    }

    /// Perto da meia-noite, a proposta atravessa o dia.
    #[test]
    fn a_proposta_atravessa_a_meia_noite() {
        let (inicio, fim) = horario_proposto(quando(2026, 8, 26, 23, 50));

        assert_eq!(
            inicio.date(),
            NaiveDate::from_ymd_opt(2026, 8, 27).expect("dia")
        );
        assert_eq!((inicio.hour(), inicio.minute()), (0, 0));
        assert_eq!((fim.hour(), fim.minute()), (0, 30));
        assert_eq!(fim.date(), inicio.date());
    }

    /// E o fim é meia hora depois do início. Sempre.
    #[test]
    fn a_duracao_proposta_e_de_meia_hora() {
        for h in 0..24u32 {
            for m in [0, 7, 29, 30, 31, 59] {
                let (inicio, fim) = horario_proposto(quando(2026, 8, 26, h, m));
                assert_eq!(
                    (fim - inicio).num_minutes(),
                    DURACAO_PADRAO_MINUTOS,
                    "às {h:02}:{m:02} a duração proposta não foi de meia hora"
                );
                assert!(fim > inicio);
            }
        }
    }

    /// O formato é o que um `datetime-local` aceita, sem segundos.
    #[test]
    fn o_campo_recebe_o_formato_que_entende() {
        let valor = para_campo(quando(2026, 8, 26, 19, 30));
        assert_eq!(valor, "2026-08-26T19:30");
        assert!(!valor.contains(':') || valor.matches(':').count() == 1);
    }
}
