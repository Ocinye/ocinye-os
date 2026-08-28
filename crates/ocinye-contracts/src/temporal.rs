//! Tempo institucional.
//!
//! # Onde isto é usado
//!
//! No Calendário e no Centro Temporal: é daqui que vem a conversão entre a hora
//! que uma pessoa escreve e o instante que a instituição guarda (ADR-0410).
//!
//! O relógio da barra superior **não** depende daqui: mostra a hora do
//! computador de quem está a ver, e não converte zonas nenhumas. A hora do
//! browser é apresentação; a hora institucional vem do Core.
//!
//! Não há atalho na raiz do crate. Os tipos alcançam-se por
//! `ocinye_contracts::temporal::…`, e escrever esse caminho é o que distingue
//! uma decisão de um acidente.
//!
//! # Duas coisas, e não uma
//!
//! Um compromisso tem **um instante** e **uma intenção**, e guardar só o
//! primeiro perde informação que depois não se recupera.
//!
//! «Reunião às 14:00 em Paris» é as duas coisas ao mesmo tempo: um ponto na
//! linha do tempo, que é o mesmo visto de Luanda ou de Camama, e uma intenção
//! humana — *catorze horas, na cidade onde as pessoas estão*. O instante
//! responde «quando é»; a zona responde «às quantas horas foi marcada».
//!
//! Guardar só o instante chega para mostrar a hora certa a cada pessoa. Não
//! chega para **editar**: quem abrir a reunião para a mudar para as 15:00 tem
//! de saber 15:00 de onde. E não chega para a recorrência, quando ela existir:
//! «todas as terças às 14:00» atravessa a mudança de hora de Verão, e sem a
//! zona não há maneira de saber se a terça seguinte é uma hora antes ou depois.
//!
//! # Porque não um offset
//!
//! `+01:00` não é uma zona. É o que uma zona vale num instante — e em Paris
//! vale `+01:00` metade do ano e `+02:00` na outra metade. Guardar o offset
//! congela a resposta de hoje e erra a de Março.
//!
//! # Um dia inteiro não é um instante
//!
//! «Prazo: 27 de Agosto» é uma **data civil**, e não um ponto na linha do
//! tempo. Forçá-la a `00:00 UTC` transforma-a num instante que, para quem está
//! a leste, cai no dia anterior — o prazo mudaria de dia consoante quem o lê.
//!
//! Por isso um evento de dia inteiro guarda a data, e não um instante.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

/// Uma zona horária IANA, validada.
///
/// Existe para que uma zona inválida seja recusada onde entra, e não descoberta
/// mais tarde por uma conversão que devolveu o que calhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimeZoneName(Tz);

impl TimeZoneName {
    /// Interpreta um nome IANA.
    ///
    /// # Errors
    ///
    /// Devolve o nome recusado quando não pertence à base IANA.
    pub fn parse(value: &str) -> Result<Self, String> {
        value
            .parse::<Tz>()
            .map(Self)
            .map_err(|_| format!("«{value}» não é uma zona horária conhecida."))
    }

    /// A zona, para conversões.
    #[must_use]
    pub const fn zone(self) -> Tz {
        self.0
    }

    /// O nome estável.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.0.name()
    }

    /// A zona da instituição, quando nada mais é sabido.
    #[must_use]
    pub const fn utc() -> Self {
        Self(Tz::UTC)
    }
}

impl TryFrom<String> for TimeZoneName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<TimeZoneName> for String {
    fn from(value: TimeZoneName) -> Self {
        value.as_str().to_owned()
    }
}

impl std::fmt::Display for TimeZoneName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Quando uma hora local não existe ou existe duas vezes.
///
/// As transições de horário de Verão fazem as duas coisas: na primavera um
/// relógio salta e há horas que não aconteceram; no outono recua e há horas que
/// acontecem duas vezes. Marcar uma reunião para uma delas é um erro humano
/// honesto, e a resposta certa é dizê-lo — não escolher em silêncio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalTimeProblem {
    /// A hora não existe nessa data e zona: o relógio saltou-a.
    DoesNotExist,
    /// A hora acontece duas vezes: o relógio recuou por cima dela.
    Ambiguous,
}

impl std::fmt::Display for LocalTimeProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DoesNotExist => f.write_str(
                "Essa hora não existe nesse dia: é a hora que o relógio salta na mudança para \
                 o horário de Verão.",
            ),
            Self::Ambiguous => f.write_str(
                "Essa hora acontece duas vezes nesse dia, na mudança de horário. Escolha uma \
                 hora antes ou depois da transição.",
            ),
        }
    }
}

/// Resolve uma hora local numa zona para o instante que ela representa.
///
/// # Errors
///
/// Devolve [`LocalTimeProblem`] quando a hora local não existe ou é ambígua na
/// zona indicada — as duas coisas que acontecem nas transições de horário.
pub fn resolve_local(
    local: NaiveDateTime,
    zone: TimeZoneName,
) -> Result<DateTime<Utc>, LocalTimeProblem> {
    match zone.zone().from_local_datetime(&local) {
        chrono::offset::LocalResult::Single(instante) => Ok(instante.with_timezone(&Utc)),
        chrono::offset::LocalResult::None => Err(LocalTimeProblem::DoesNotExist),
        chrono::offset::LocalResult::Ambiguous(_, _) => Err(LocalTimeProblem::Ambiguous),
    }
}

/// Mostra um instante na zona em que foi marcado.
#[must_use]
pub fn in_zone(instante: DateTime<Utc>, zone: TimeZoneName) -> NaiveDateTime {
    instante.with_timezone(&zone.zone()).naive_local()
}

/// Quando uma coisa acontece.
///
/// # Porque não é sempre um instante
///
/// Um dia inteiro é uma data civil, e uma data civil não tem hora. Guardá-la
/// como `00:00 UTC` fá-la-ia cair no dia anterior para quem está a leste — e um
/// prazo que muda de dia consoante quem o lê não é um prazo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Occurrence {
    /// Um intervalo com hora, ancorado a um instante e a uma intenção.
    Timed {
        /// O instante em que começa.
        starts_at: DateTime<Utc>,
        /// O instante em que acaba.
        ends_at: DateTime<Utc>,
        /// A zona em que foi marcado, e onde a hora escrita faz sentido.
        timezone: TimeZoneName,
    },
    /// Um ou mais dias civis, sem hora.
    ///
    /// O intervalo é **meio-aberto**: `[starts_on, ends_before)`. Um evento de
    /// um dia é `24 → 25`, e não `24 → 24`.
    ///
    /// # Porque não «o último dia, inclusive»
    ///
    /// Porque a alternativa obriga toda a gente a lembrar-se de somar um dia, e
    /// mais cedo ou mais tarde alguém não soma. Um intervalo inclusivo também
    /// não sabe representar a duração zero, e faz a aritmética de sobreposição
    /// precisar de um `+1` em cada comparação — que é onde os erros de um dia
    /// nascem. Meio-aberto compõe-se: o fim de um é o princípio do seguinte,
    /// sem lacuna nem sobreposição.
    AllDay {
        /// O primeiro dia.
        starts_on: NaiveDate,
        /// O dia **a seguir** ao último. Exclusivo.
        ends_before: NaiveDate,
    },
}

impl Occurrence {
    /// O instante a partir do qual isto conta, para ordenar e comparar.
    ///
    /// Um dia inteiro entra na ordenação pela meia-noite da sua zona de
    /// referência. É uma aproximação **para ordenar**, e nunca para decidir em
    /// que dia a coisa cai: essa resposta está na data, e é exacta.
    #[must_use]
    pub fn ordering_instant(&self, reference: TimeZoneName) -> DateTime<Utc> {
        match self {
            Self::Timed { starts_at, .. } => *starts_at,
            Self::AllDay { starts_on, .. } => reference
                .zone()
                .from_local_datetime(&starts_on.and_hms_opt(0, 0, 0).unwrap_or_default())
                .earliest()
                .map_or_else(Utc::now, |d| d.with_timezone(&Utc)),
        }
    }

    /// Se isto é um dia inteiro.
    #[must_use]
    pub const fn is_all_day(&self) -> bool {
        matches!(self, Self::AllDay { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uma_zona_desconhecida_e_recusada_onde_entra() {
        assert!(TimeZoneName::parse("Europe/Paris").is_ok());
        assert!(TimeZoneName::parse("Africa/Luanda").is_ok());
        assert!(TimeZoneName::parse("UTC").is_ok());

        for inventada in ["Europe/Atlantis", "+01:00", "CET+1", "", "Luanda"] {
            assert!(
                TimeZoneName::parse(inventada).is_err(),
                "«{inventada}» foi aceite como zona horária"
            );
        }
    }

    /// A mesma hora local em duas zonas é um instante diferente.
    ///
    /// É o teste que justifica guardar a zona: sem ela, «14:00» não identifica
    /// momento nenhum.
    #[test]
    fn a_mesma_hora_local_em_zonas_diferentes_e_outro_instante() {
        let local = NaiveDate::from_ymd_opt(2026, 8, 25)
            .expect("data")
            .and_hms_opt(14, 0, 0)
            .expect("hora");

        let paris = resolve_local(local, TimeZoneName::parse("Europe/Paris").expect("zona"))
            .expect("instante");
        let luanda = resolve_local(local, TimeZoneName::parse("Africa/Luanda").expect("zona"))
            .expect("instante");

        assert_ne!(
            paris, luanda,
            "14:00 em Paris e 14:00 em Luanda deram o mesmo instante"
        );
    }

    /// O instante sobrevive à viagem: é a propriedade que o §16 pede.
    #[test]
    fn uma_reuniao_marcada_em_paris_e_o_mesmo_instante_vista_de_luanda() {
        let paris = TimeZoneName::parse("Europe/Paris").expect("zona");
        let luanda = TimeZoneName::parse("Africa/Luanda").expect("zona");

        let local = NaiveDate::from_ymd_opt(2026, 8, 25)
            .expect("data")
            .and_hms_opt(14, 0, 0)
            .expect("hora");
        let instante = resolve_local(local, paris).expect("instante");

        // Em Paris continua a ler-se 14:00 — é onde foi marcada.
        assert_eq!(in_zone(instante, paris), local);

        // Em Luanda lê-se outra hora, e é o mesmo momento.
        let visto_de_luanda = in_zone(instante, luanda);
        assert_ne!(visto_de_luanda, local);
        assert_eq!(
            resolve_local(visto_de_luanda, luanda).expect("instante"),
            instante,
            "converter para Luanda e de volta mudou o momento"
        );
    }

    /// As horas que não existem e as que existem duas vezes são recusadas.
    ///
    /// Em 2026 a Europa passa ao horário de Verão a 29 de Março: às 02:00 o
    /// relógio salta para as 03:00, e as 02:30 não aconteceram. A 25 de Outubro
    /// recua, e as 02:30 acontecem duas vezes.
    #[test]
    fn as_horas_das_transicoes_sao_recusadas_em_vez_de_escolhidas_em_silencio() {
        let paris = TimeZoneName::parse("Europe/Paris").expect("zona");

        let inexistente = NaiveDate::from_ymd_opt(2026, 3, 29)
            .expect("data")
            .and_hms_opt(2, 30, 0)
            .expect("hora");
        assert_eq!(
            resolve_local(inexistente, paris),
            Err(LocalTimeProblem::DoesNotExist),
            "uma hora que o relógio salta foi aceite"
        );

        let ambigua = NaiveDate::from_ymd_opt(2026, 10, 25)
            .expect("data")
            .and_hms_opt(2, 30, 0)
            .expect("hora");
        assert_eq!(
            resolve_local(ambigua, paris),
            Err(LocalTimeProblem::Ambiguous),
            "uma hora que acontece duas vezes foi aceite sem escolha"
        );
    }

    /// Um offset fixo não substitui uma zona.
    ///
    /// Paris vale `+01:00` no Inverno e `+02:00` no Verão. Guardar o offset
    /// congela a resposta de um dia e erra a do outro.
    #[test]
    fn um_offset_fixo_nao_substitui_uma_zona() {
        let paris = TimeZoneName::parse("Europe/Paris").expect("zona");
        let meio_dia = |mes, dia| {
            NaiveDate::from_ymd_opt(2026, mes, dia)
                .expect("data")
                .and_hms_opt(12, 0, 0)
                .expect("hora")
        };

        let inverno = resolve_local(meio_dia(1, 15), paris).expect("instante");
        let verao = resolve_local(meio_dia(7, 15), paris).expect("instante");

        // Meio-dia local nos dois casos, e offsets diferentes em UTC.
        assert_eq!(inverno.time().to_string(), "11:00:00");
        assert_eq!(verao.time().to_string(), "10:00:00");
    }

    /// Um dia inteiro é uma data, e não uma meia-noite.
    #[test]
    fn um_dia_inteiro_nao_e_um_instante() {
        let dia = NaiveDate::from_ymd_opt(2026, 8, 27).expect("data");
        let prazo = Occurrence::AllDay {
            starts_on: dia,
            ends_before: dia.succ_opt().expect("o dia seguinte"),
        };

        assert!(prazo.is_all_day());

        // A data é a mesma independentemente da zona de referência: é isso que
        // se perderia ao guardá-la como `00:00 UTC`.
        let Occurrence::AllDay { starts_on, .. } = prazo else {
            panic!("devia ser um dia inteiro");
        };
        assert_eq!(starts_on, dia);
    }
}
