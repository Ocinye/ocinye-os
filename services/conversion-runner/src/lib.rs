//! O catálogo **fechado** de perfis de conversão.
//!
//! É o coração da fronteira: o worker pede um perfil pelo **nome**, e nunca um
//! comando. O runner só sabe correr os perfis que estão aqui, com o tecto de
//! recursos que aqui está escrito; o `ocinye-convert`, dentro do contentor
//! descartável, só sabe executar os que aqui estão. Um nome que o runner aceite
//! e o conversor não reconheça — ou o contrário — é um erro que este módulo
//! único torna impossível.
//!
//! Acrescentar um formato é acrescentar uma linha a [`PROFILES`] e um ramo em
//! `ocinye-convert`. Nada no worker ganha autoridade nova: continua a pedir um
//! nome de uma lista fechada.

/// Um perfil de conversão: o que o conversor faz, e o tecto que o runner dá ao
/// contentor descartável que o corre.
///
/// Os limites viajam **com** o perfil de propósito. Um rasterizador de PDF e um
/// LibreOffice não têm o mesmo apetite, e o tecto de um não pode ser o do outro
/// por descuido — cada perfil declara o seu.
#[derive(Debug, Clone, Copy)]
pub struct Profile {
    /// O nome estável. É isto que o worker pede e o URL nomeia.
    pub name: &'static str,
    /// O prazo de parede da conversão inteira, em segundos. Esgotado, o runner
    /// mata e remove o contentor.
    pub timeout_secs: u64,
    /// O tecto de memória, tal como `docker run --memory` o entende.
    pub memory: &'static str,
    /// O tecto de processos, para `docker run --pids-limit`.
    pub pids: u32,
    /// O tecto de CPU, para `docker run --cpus`.
    pub cpus: &'static str,
}

/// Os perfis que existem. Fechado por decisão: uma conversão que não esteja aqui
/// não acontece.
///
/// Os tectos diferem por peso do parser: o `pdftoppm` é leve; o LibreOffice
/// arranca devagar, come memória e forqueia processos-filho, por isso leva prazo,
/// memória e `pids` maiores; o `ffmpeg` fica no meio.
pub const PROFILES: &[Profile] = &[
    Profile {
        name: "pdf-thumbnail",
        timeout_secs: 25,
        memory: "768m",
        pids: 128,
        cpus: "1",
    },
    Profile {
        name: "office-thumbnail",
        timeout_secs: 90,
        memory: "1024m",
        pids: 512,
        cpus: "1",
    },
    Profile {
        name: "video-thumbnail",
        timeout_secs: 45,
        memory: "768m",
        pids: 256,
        cpus: "1",
    },
];

/// Resolve um perfil pelo nome, ou `None` se não existir.
#[must_use]
pub fn profile(name: &str) -> Option<&'static Profile> {
    PROFILES.iter().find(|candidato| candidato.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_perfil_desconhecido_nao_resolve() {
        assert!(profile("pdf-thumbnail").is_some());
        assert!(profile("executar-comando-arbitrario").is_none());
        assert!(profile("").is_none());
    }

    #[test]
    fn cada_perfil_tem_um_tecto_util() {
        for p in PROFILES {
            assert!(p.timeout_secs > 0, "{}: prazo nulo", p.name);
            assert!(p.pids >= 1, "{}: sem processos", p.name);
            assert!(!p.memory.is_empty(), "{}: sem tecto de memória", p.name);
            assert!(!p.cpus.is_empty(), "{}: sem tecto de CPU", p.name);
        }
    }

    #[test]
    fn os_nomes_dos_perfis_nao_se_repetem() {
        for (i, a) in PROFILES.iter().enumerate() {
            for b in &PROFILES[i + 1..] {
                assert_ne!(a.name, b.name, "perfil repetido: {}", a.name);
            }
        }
    }
}
