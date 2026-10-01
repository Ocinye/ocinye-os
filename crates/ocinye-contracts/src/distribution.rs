//! D010 · Distribuições de uma Instância (contrato tipado, Claude Design; integrado pela Code).
//!
//! Substitui a suposição «um perfil por Instância» (ADR-0014 §1, `organisations.profile`)
//! por um conjunto fechado e não vazio de Distribuições activadas (ADR-0019).
//!
//! Três conceitos que nunca se fundem:
//!   disponíveis  = `Distribution::ALL` (as quatro do produto)
//!   activadas    = `EnabledDistributions` (por Instância, 1..4)
//!   acessíveis   = activadas ∩ `DistributionAccess` do membro (decisão do Core)
//!
//! Nada aqui é autorização: dentro da Distribuição, o RBAC continua a decidir (ADR-0100).

use serde::{Deserialize, Serialize};

use crate::application::ApplicationId;

/// As quatro Distribuições do Ocinye OS. Fechado: não há quinta, nem texto livre.
///
/// Migração: `InstanceProfile` passa a `pub type InstanceProfile = Distribution;` durante
/// uma versão, para que `activates` e os testes D009 não mudem de nome no mesmo passo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Distribution {
    /// Investigação: todas as aplicações (o Ocinye como a Ocinye o usa).
    Research,
    /// Empresa: comunicação, projectos e unidades.
    Business,
    /// Pessoal: uma pessoa e as suas coisas.
    Personal,
    /// Educação: como a empresa, mais conhecimento e bibliografia.
    Education,
}

impl Distribution {
    /// As disponíveis, na ordem de apresentação (D009).
    pub const ALL: [Distribution; 4] = [
        Self::Research,
        Self::Business,
        Self::Personal,
        Self::Education,
    ];

    #[must_use]
    /// O identificador persistido.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Business => "business",
            Self::Personal => "personal",
            Self::Education => "education",
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Research => 1,
            Self::Business => 2,
            Self::Personal => 4,
            Self::Education => 8,
        }
    }

    /// Se esta aplicação começa activa nesta Distribuição (ADR-0014 §4; ADR-0019 §9).
    ///
    /// Uma aplicação essencial está sempre activa, em qualquer perfil.
    #[must_use]
    pub const fn activates(self, app: ApplicationId) -> bool {
        use ApplicationId as A;
        if !app.is_optional() {
            return true;
        }
        match self {
            Self::Research => true,
            Self::Business => matches!(
                app,
                A::Notes
                    | A::Calendar
                    | A::Mail
                    | A::Prompt
                    | A::Messages
                    | A::Activity
                    | A::Audit
                    | A::Units
                    | A::Projects
                    | A::Terminal
                    | A::Browser
            ),
            Self::Education => matches!(
                app,
                A::Notes
                    | A::Calendar
                    | A::Mail
                    | A::Prompt
                    | A::Messages
                    | A::Activity
                    | A::Audit
                    | A::Units
                    | A::Projects
                    | A::Knowledge
                    | A::Bibliography
                    | A::Terminal
                    | A::Browser
            ),
            Self::Personal => matches!(
                app,
                A::Notes | A::Calendar | A::Mail | A::Prompt | A::Terminal | A::Browser
            ),
        }
    }

    /// Se a estrutura inicial de unidades é semeada (ADR-0014 §7).
    #[must_use]
    pub const fn seeds_initial_units(self) -> bool {
        matches!(self, Self::Research)
    }
}

impl TryFrom<String> for Distribution {
    type Error = UnknownDistribution;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        v.parse()
    }
}

impl std::str::FromStr for Distribution {
    type Err = UnknownDistribution;

    /// Code: aceita espaços à volta (como o `InstanceProfile` da D009), e mais nada —
    /// nem maiúsculas, nem sinónimos; o desconhecido é recusado, nunca Research.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|d| d.as_str() == value.trim())
            .ok_or_else(|| UnknownDistribution(value.to_owned()))
    }
}

impl From<Distribution> for String {
    fn from(d: Distribution) -> Self {
        d.as_str().to_owned()
    }
}

/// Um identificador que não é uma das quatro. Recusa-se; nunca cai em Research.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDistribution(pub String);

impl std::fmt::Display for UnknownDistribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Distribuição desconhecida: «{}» (research, business, personal ou education)",
            self.0
        )
    }
}

impl std::error::Error for UnknownDistribution {}

/// Um conjunto de Distribuições. Não vazio por construção quando é o das activadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DistributionSet(u8);

impl DistributionSet {
    #[must_use]
    /// O conjunto vazio.
    pub const fn empty() -> Self {
        Self(0)
    }
    #[must_use]
    /// Com esta Distribuição.
    pub const fn with(self, d: Distribution) -> Self {
        Self(self.0 | d.bit())
    }
    #[must_use]
    /// Sem esta Distribuição.
    pub const fn without(self, d: Distribution) -> Self {
        Self(self.0 & !d.bit())
    }
    #[must_use]
    /// Se a contém.
    pub const fn contains(self, d: Distribution) -> bool {
        self.0 & d.bit() != 0
    }
    #[must_use]
    /// Intersecção (ex.: activadas ∩ acesso do membro).
    pub const fn intersect(self, o: Self) -> Self {
        Self(self.0 & o.0)
    }
    #[must_use]
    /// Quantas.
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }
    #[must_use]
    /// Se não tem nenhuma.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// As que contém, pela ordem de `Distribution::ALL`.
    pub fn iter(self) -> impl Iterator<Item = Distribution> {
        Distribution::ALL
            .into_iter()
            .filter(move |d| self.contains(*d))
    }
}

/// As Distribuições activadas de uma Instância. Invariante: 1..=4 (ADR-0019 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnabledDistributions(DistributionSet);

impl EnabledDistributions {
    /// Recusa o conjunto vazio: uma Instância sem Distribuição não tem Desktop.
    pub fn new(set: DistributionSet) -> Result<Self, DistributionRefusal> {
        if set.is_empty() {
            Err(DistributionRefusal::LastEnabled)
        } else {
            Ok(Self(set))
        }
    }
    #[must_use]
    /// O conjunto.
    pub const fn set(self) -> DistributionSet {
        self.0
    }
    /// Desactivar a última é recusado — não há «estado de manutenção» implícito.
    pub fn disable(self, d: Distribution) -> Result<Self, DistributionRefusal> {
        if !self.0.contains(d) {
            return Err(DistributionRefusal::NotEnabled(d));
        }
        Self::new(self.0.without(d))
    }
    #[must_use]
    /// Activar acrescenta; nunca falha.
    pub const fn enable(self, d: Distribution) -> Self {
        Self(self.0.with(d))
    }
}

/// Estado persistido de uma Distribuição numa Instância. Desactivar guarda tudo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionState {
    /// Activada: entra-se.
    Enabled,
    /// Desactivada: entrada nova recusada; disposições, fixações e acessos guardados.
    Disabled,
}

/// Recusas tipadas (ADR-0111): o Workspace traduz cada uma num estado honesto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "distribution", rename_all = "snake_case")]
pub enum DistributionRefusal {
    /// Tentativa de ficar com zero activadas.
    LastEnabled,
    /// A Distribuição não está activada.
    NotEnabled(Distribution),
    /// Retirar o acesso deixaria a Instância sem nenhum administrador que entre.
    LastAdministratorAccess,
}

/// Decisão do Core à pergunta «este membro pode entrar nesta Distribuição?».
/// Não é um papel e não substitui o RBAC dentro dela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryDecision {
    /// Pode entrar.
    Allowed,
    /// A Instância não a tem activada (S12, S39).
    NotEnabled,
    /// Activada, mas o membro não tem acesso (S11, S18).
    NoAccess,
}

/// O que a entrada mostra depois da autenticação num ponto genérico (S09/S10/direct).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryResolution {
    /// Zero acessíveis: S10. Não é palavra-passe errada, nem 404, nem Core em baixo.
    NoneAccessible,
    /// Uma: entra directamente, sem seletor de uma opção.
    Direct(Distribution),
    /// Várias: seletor só com estas (nunca contextos).
    Choose(DistributionSet),
}

impl EntryResolution {
    #[must_use]
    /// A decisão de entrada para um conjunto de acessíveis.
    pub fn from_accessible(accessible: DistributionSet) -> Self {
        match accessible.len() {
            0 => Self::NoneAccessible,
            1 => Self::Direct(accessible.iter().next().expect("len 1")),
            _ => Self::Choose(accessible),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quatro_fechadas_e_o_desconhecido_nunca_e_research() {
        assert_eq!(Distribution::ALL.len(), 4);
        for d in Distribution::ALL {
            assert_eq!(d.as_str().parse::<Distribution>(), Ok(d));
        }
        for s in [
            "",
            "Research",
            "RESEARCH",
            "science",
            "../research",
            "research,business",
        ] {
            assert!(s.parse::<Distribution>().is_err(), "«{s}»");
        }
        assert_eq!(
            " research ".parse::<Distribution>(),
            Ok(Distribution::Research)
        );
    }

    #[test]
    fn as_activadas_nunca_ficam_vazias() {
        let r = EnabledDistributions::new(DistributionSet::empty().with(Distribution::Research))
            .expect("uma");
        assert_eq!(
            r.disable(Distribution::Research),
            Err(DistributionRefusal::LastEnabled)
        );
        assert_eq!(
            r.disable(Distribution::Business),
            Err(DistributionRefusal::NotEnabled(Distribution::Business))
        );
        let rb = r.enable(Distribution::Business);
        assert_eq!(rb.set().len(), 2);
        let b = rb.disable(Distribution::Research).expect("fica Business");
        assert_eq!(b.set().iter().collect::<Vec<_>>(), [Distribution::Business]);
        assert!(EnabledDistributions::new(DistributionSet::empty()).is_err());
    }

    #[test]
    fn a_entrada_e_zero_uma_ou_varias() {
        let s = DistributionSet::empty();
        assert_eq!(
            EntryResolution::from_accessible(s),
            EntryResolution::NoneAccessible
        );
        let one = s.with(Distribution::Education);
        assert_eq!(
            EntryResolution::from_accessible(one),
            EntryResolution::Direct(Distribution::Education)
        );
        let two = one.with(Distribution::Business);
        assert_eq!(
            EntryResolution::from_accessible(two),
            EntryResolution::Choose(two)
        );
        // Activadas ∩ acesso: nunca as que o membro não tem.
        let enabled = DistributionSet::empty()
            .with(Distribution::Research)
            .with(Distribution::Business)
            .with(Distribution::Personal)
            .with(Distribution::Education);
        let access = DistributionSet::empty()
            .with(Distribution::Business)
            .with(Distribution::Education);
        assert_eq!(
            enabled.intersect(access).iter().collect::<Vec<_>>(),
            [Distribution::Business, Distribution::Education]
        );
    }
}
