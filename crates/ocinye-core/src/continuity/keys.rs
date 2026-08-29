//! O material criptográfico, e o que acontece a cada peça numa migração.
//!
//! # A pergunta que isto responde
//!
//! > **Existe estado persistido que só pode ser interpretado com material
//! > criptográfico que não vive na base de dados?**
//!
//! Se existir e não viajar, o restore passa **estruturalmente** — as linhas
//! chegam, as identidades coincidem, o verificador diz que sim — e parte da
//! instituição fica inutilizável. É a pior forma de falhar, porque parece
//! sucesso.
//!
//! # A distinção que custa caro confundir
//!
//! | | |
//! |---|---|
//! | **Durável** | interpreta história já escrita. Perdê-la é perder essa história. **Tem de viajar.** |
//! | **Substituível** | dá acesso a um serviço. **Roda-se** no servidor novo; copiá-la alarga a exposição sem preservar nada. |
//!
//! Uma senha de fornecedor pode ser trocada por outra e tudo continua a
//! funcionar. Uma chave de selagem não: sem ela, `mailbox_credentials` é ruído
//! com a forma certa, e nenhuma chave nova a reconstrói.
//!
//! # Porque isto é um inventário fechado
//!
//! Porque a resposta «só há uma chave» é verdadeira hoje. O que este módulo
//! garante é que deixe de ser verdadeira **em voz alta**: qualquer coluna nova
//! que guarde criptograma, e qualquer sítio novo que sele, obrigam a declarar
//! aqui o que interpreta esse estado.

use serde::{Deserialize, Serialize};

/// O que acontece a uma peça de material criptográfico numa migração.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destino {
    /// Viaja. Sem ela, estado durável já escrito fica ilegível.
    Duravel,
    /// Roda-se no destino. Não interpreta nada do que ficou guardado.
    Substituivel,
}

impl Destino {
    /// A representação estável, para documentação e manifestos.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Duravel => "DURABLE_CRYPTOGRAPHIC_MATERIAL",
            Self::Substituivel => "REPLACEABLE_DEPLOYMENT_CREDENTIAL",
        }
    }
}

/// Uma peça de material criptográfico ou de credencial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    /// A variável de ambiente que a transporta.
    pub variavel: &'static str,
    /// O que acontece numa migração.
    pub destino: Destino,
    /// O estado durável que esta peça interpreta, se interpretar algum.
    ///
    /// `None` para as substituíveis: não interpretam nada, e é isso que as
    /// torna substituíveis.
    pub interpreta: Option<&'static str>,
    /// Porquê.
    pub porque: &'static str,
}

/// Todo o material criptográfico e credencial que a instalação lê.
///
/// # Porque as substituíveis também estão aqui
///
/// Porque «não está na lista» seria indistinguível de «ninguém pensou nisso».
/// Uma credencial de fornecedor tem de estar declarada como substituível para
/// que a ausência dela do canal seguro seja uma decisão, e não um esquecimento.
pub const MATERIAL: &[Material] = &[
    Material {
        variavel: "OCINYE_MAIL_KEY",
        destino: Destino::Duravel,
        interpreta: Some("mailbox_credentials"),
        porque: "Sela a senha de caixa de cada membro com ChaCha20-Poly1305. A \
                 chave não está na base: sem ela, as linhas chegam íntegras e \
                 ilegíveis, e nenhuma chave nova as reconstrói. É a única peça \
                 desta lista que interpreta história já escrita.",
    },
    Material {
        variavel: "OCINYE_DATABASE_URL",
        destino: Destino::Substituivel,
        interpreta: None,
        porque: "Contém a senha do PostgreSQL do servidor. O papel da base é do \
                 servidor, não da instituição — é por isso que o dump se faz \
                 com `--no-owner`.",
    },
    Material {
        variavel: "OCINYE_REDIS_URL",
        destino: Destino::Substituivel,
        interpreta: None,
        porque: "Coordenação efémera. O Redis do servidor novo arranca vazio, e \
                 é essa a prova de que não é fonte de verdade.",
    },
    Material {
        variavel: "OCINYE_STORAGE_ACCESS_KEY",
        destino: Destino::Substituivel,
        interpreta: None,
        porque: "Acesso ao bucket. Os objectos não estão cifrados por esta \
                 chave: ela abre a porta, não o conteúdo.",
    },
    Material {
        variavel: "OCINYE_STORAGE_SECRET_KEY",
        destino: Destino::Substituivel,
        interpreta: None,
        porque: "O par da anterior. Rodar no destino é preferível a copiar.",
    },
    Material {
        variavel: "OCINYE_MAIL_PASSWORD",
        destino: Destino::Substituivel,
        interpreta: None,
        porque: "Senha da conta institucional no fornecedor de correio. \
                 Trocá-la não torna ilegível nada do que está guardado.",
    },
];

/// As colunas do esquema que guardam criptograma.
///
/// Escritas aqui para que o teste de cobertura tenha contra o que confrontar o
/// esquema. Cada uma tem de nomear a variável que a interpreta.
#[cfg(test)]
const CRIPTOGRAMA_NO_ESQUEMA: &[(&str, &str)] = &[("mailbox_credentials", "OCINYE_MAIL_KEY")];

/// O que uma instalação consegue **ler** do estado durável selado que tem.
///
/// Separado do comando que o imprime porque é uma decisão sobre estado
/// institucional, e porque o caso que interessa — estado selado presente e
/// chave ausente — é precisamente aquele que nunca acontece na máquina de quem
/// escreveu o código, e sempre na do servidor novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Legibilidade {
    /// Não há estado selado, e não há chave. Nada foi verificado.
    ///
    /// **Não é `PASS`.** É a ausência de pergunta.
    NadaParaLer,
    /// Há estado selado e não há chave.
    ///
    /// O caso catastrófico, e o único que os outros verificadores deixam
    /// passar: as linhas chegaram íntegras e ninguém as consegue ler.
    IlegivelSemChave {
        /// Quantas linhas chegaram assim.
        seladas: i64,
    },
    /// Há chave e não há estado selado. Nada foi verificado.
    ChaveSemEstado,
    /// Tudo o que estava selado abriu.
    Legivel {
        /// Quantas.
        abriram: usize,
    },
    /// Alguma coisa não abriu.
    Ilegivel {
        /// Quantas recusaram.
        recusadas: usize,
        /// De quantas.
        total: usize,
    },
}

impl Legibilidade {
    /// Se isto conta como uma verificação bem-sucedida.
    ///
    /// «Nada para ler» **não** conta: um verificador que não encontrou o que
    /// devia observar não teve sucesso, observou zero.
    #[must_use]
    pub const fn verificou(&self) -> bool {
        matches!(self, Self::Legivel { .. })
    }

    /// Se isto deve terminar o processo com código não-zero.
    #[must_use]
    pub const fn e_falha(&self) -> bool {
        matches!(self, Self::IlegivelSemChave { .. } | Self::Ilegivel { .. })
    }
}

/// Decide a legibilidade a partir do que se observou.
///
/// `recusadas` conta as linhas que não abriram; só é significativo quando há
/// chave.
#[must_use]
pub const fn legibilidade(tem_chave: bool, seladas: i64, recusadas: usize) -> Legibilidade {
    if !tem_chave {
        if seladas == 0 {
            return Legibilidade::NadaParaLer;
        }
        return Legibilidade::IlegivelSemChave { seladas };
    }
    if seladas == 0 {
        return Legibilidade::ChaveSemEstado;
    }
    let total = seladas as usize;
    if recusadas == 0 {
        return Legibilidade::Legivel { abriram: total };
    }
    Legibilidade::Ilegivel { recusadas, total }
}

/// O que tem de viajar por canal próprio.
#[must_use]
pub fn viaja_por_canal_proprio() -> Vec<&'static Material> {
    MATERIAL
        .iter()
        .filter(|m| m.destino == Destino::Duravel)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O durável interpreta alguma coisa; o substituível não interpreta nada.
    ///
    /// É a definição, e escrevê-la como teste impede que alguém marque uma
    /// chave de selagem como substituível por ela «também ser um segredo».
    #[test]
    fn o_destino_e_a_consequencia_de_interpretar_ou_nao() {
        for material in MATERIAL {
            match material.destino {
                Destino::Duravel => assert!(
                    material.interpreta.is_some(),
                    "`{}` viaja por ser durável e não diz o que interpreta; se \
                     não interpreta nada, é substituível",
                    material.variavel
                ),
                Destino::Substituivel => assert!(
                    material.interpreta.is_none(),
                    "`{}` interpreta `{:?}` e está marcada como substituível. \
                     Rodá-la no destino tornaria esse estado ilegível para \
                     sempre",
                    material.variavel,
                    material.interpreta
                ),
            }
        }
    }

    /// Cada peça diz porquê, e a razão lê-se.
    #[test]
    fn nenhuma_decisao_e_muda() {
        for material in MATERIAL {
            assert!(
                material.porque.split_whitespace().count() >= 10,
                "`{}` está classificada sem razão que se leia",
                material.variavel
            );
        }
    }

    /// Toda a coluna de criptograma do esquema tem quem a interprete declarado.
    ///
    /// # O defeito que isto guarda
    ///
    /// Alguém acrescenta uma segunda coisa selada — um token de integração, um
    /// segredo de webhook — com uma chave nova. O restore continua a passar,
    /// porque as linhas chegam. E a coisa nova chega ilegível, sem que nada o
    /// diga, até ao dia em que alguém a tenta usar.
    #[test]
    fn toda_a_coluna_cifrada_tem_chave_declarada() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("migrations");

        let mut ficheiros: Vec<_> = std::fs::read_dir(&raiz)
            .expect("migrations")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "sql"))
            .collect();
        ficheiros.sort();
        assert!(
            !ficheiros.is_empty(),
            "não há migrations para observar; isto seria verde por vazio"
        );

        // A tabela é a que estiver aberta quando a coluna aparece. Um
        // `CREATE TABLE` novo fecha a anterior.
        let mut com_criptograma: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for ficheiro in &ficheiros {
            let sql = std::fs::read_to_string(ficheiro).expect("ler migration");
            let mut tabela = String::new();
            for linha in sql.lines() {
                let limpa = linha.trim();
                if let Some(resto) = limpa.strip_prefix("CREATE TABLE ") {
                    tabela = resto
                        .trim_start_matches("IF NOT EXISTS ")
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .trim_end_matches('(')
                        .to_owned();
                    continue;
                }
                // Só a declaração de coluna conta. Um comentário que fale de
                // nonces não guarda nenhum.
                if limpa.starts_with("--") || tabela.is_empty() {
                    continue;
                }
                let Some(coluna) = limpa.split_whitespace().next() else {
                    continue;
                };
                if matches!(coluna, "ciphertext" | "nonce" | "sealed" | "encrypted") {
                    com_criptograma.insert(tabela.clone());
                }
            }
        }

        // Controlo positivo: se a leitura se partir e não encontrar nada, o
        // teste não pode passar por vazio.
        assert!(
            com_criptograma.contains("mailbox_credentials"),
            "a leitura das migrations não encontrou sequer `mailbox_credentials`, \
             que tem `nonce` e `ciphertext`. O varrimento partiu-se, e um verde \
             aqui não significaria nada. Encontrou: {com_criptograma:?}"
        );

        let declaradas: std::collections::BTreeSet<&str> =
            CRIPTOGRAMA_NO_ESQUEMA.iter().map(|(t, _)| *t).collect();
        let sem_chave: Vec<&String> = com_criptograma
            .iter()
            .filter(|t| !declaradas.contains(t.as_str()))
            .collect();
        assert!(
            sem_chave.is_empty(),
            "estas tabelas guardam criptograma e ninguém declarou que chave as \
             interpreta: {sem_chave:?}. Um restore que não leve essa chave passa \
             estruturalmente e entrega estado ilegível"
        );

        // E a chave nomeada tem de existir no inventário, marcada como durável.
        for (tabela, variavel) in CRIPTOGRAMA_NO_ESQUEMA {
            let material = MATERIAL
                .iter()
                .find(|m| m.variavel == *variavel)
                .unwrap_or_else(|| panic!("`{variavel}` não está em `MATERIAL`"));
            assert_eq!(
                material.destino,
                Destino::Duravel,
                "`{tabela}` é interpretada por `{variavel}`, que está marcada \
                 como substituível"
            );
        }
    }

    /// Não há um segundo sítio a selar com uma chave que ninguém declarou.
    ///
    /// O teste acima olha para o esquema; este olha para o código. Uma coisa
    /// selada que nunca chegue a uma coluna chamada `ciphertext` — guardada em
    /// `JSONB`, por exemplo — escaparia ao primeiro.
    ///
    /// A leitura é da **vizinhança da chamada**, e não do ficheiro: `config.rs`
    /// nomeia todas as variáveis da instalação, e um varrimento por ficheiro
    /// concluiria que a instituição sela com a senha do Redis.
    #[test]
    fn so_existe_uma_chave_de_selagem_no_codigo() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut chaves: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut ficheiros = 0_usize;
        let mut construcoes = 0_usize;

        fn percorrer(
            dir: &std::path::Path,
            chaves: &mut std::collections::BTreeSet<String>,
            ficheiros: &mut usize,
            construcoes: &mut usize,
        ) {
            for entrada in std::fs::read_dir(dir).expect("ler src").flatten() {
                let caminho = entrada.path();
                if caminho.is_dir() {
                    percorrer(&caminho, chaves, ficheiros, construcoes);
                    continue;
                }
                if !caminho.extension().is_some_and(|e| e == "rs") {
                    continue;
                }
                // Este próprio ficheiro nomeia a chamada para a poder procurar.
                // Contá-lo seria o guarda a observar-se a si mesmo.
                if caminho.file_name().is_some_and(|n| n == "keys.rs") {
                    continue;
                }
                *ficheiros += 1;
                let fonte = std::fs::read_to_string(&caminho).expect("ler fonte");
                let linhas: Vec<&str> = fonte.lines().collect();
                for (i, linha) in linhas.iter().enumerate() {
                    if !linha.contains("SealingKey::from_base64") {
                        continue;
                    }
                    // Fora de `#[cfg(test)]` não há construção de chave a
                    // partir de `generate()`; dentro há, e é fixture.
                    if linha.contains("SealingKey::generate()") {
                        continue;
                    }
                    *construcoes += 1;
                    // A variável que alimenta a chave é a **primeira lida
                    // acima** da chamada, e não qualquer uma que apareça por
                    // perto: em `config.rs` a leitura da senha de correio está
                    // quatro linhas antes, e uma janela concluiria que a
                    // instituição sela com ela.
                    let inicio = i.saturating_sub(10);
                    for anterior in linhas[inicio..=i].iter().rev() {
                        let Some(nome) = MATERIAL
                            .iter()
                            .map(|m| m.variavel)
                            .find(|v| anterior.contains(v))
                        else {
                            continue;
                        };
                        chaves.insert(nome.to_owned());
                        break;
                    }
                }
            }
        }
        percorrer(&raiz, &mut chaves, &mut ficheiros, &mut construcoes);

        assert!(
            ficheiros > 50,
            "só {ficheiros} ficheiros percorridos; o varrimento partiu-se e este \
             teste seria verde por não ter olhado"
        );
        // Controlo positivo: se nenhuma construção for encontrada, o conjunto
        // vazio seria trivialmente um subconjunto de tudo.
        assert!(
            construcoes > 0,
            "não foi encontrada nenhuma construção de `SealingKey` no código de \
             produção. Ou deixou de haver selagem — e então `mailbox_credentials` \
             mudou —, ou este varrimento partiu-se e o verde não significa nada"
        );

        let duraveis: std::collections::BTreeSet<String> = viaja_por_canal_proprio()
            .into_iter()
            .map(|m| m.variavel.to_owned())
            .collect();
        assert!(
            chaves.is_subset(&duraveis) && !chaves.is_empty(),
            "o código sela com {chaves:?}, e o inventário declara {duraveis:?} \
             como durável. Uma chave que sele estado e não viaje entrega um \
             restore que passa e não se lê"
        );
    }

    /// Estado selado sem chave é falha, e não «nada a fazer».
    ///
    /// # O defeito que isto guarda
    ///
    /// É o único estado que `verify-snapshot` e `verify-objects` deixam passar
    /// os dois: as linhas chegaram, os bytes chegaram, e o que chegou não se
    /// lê. Um comando que tratasse a chave em falta como «não configurado» —
    /// como o correio faz, e correctamente, numa instalação nova — declararia
    /// saudável um restore que perdeu memória institucional.
    #[test]
    fn estado_selado_sem_chave_e_falha() {
        let v = legibilidade(false, 318, 0);
        assert_eq!(v, Legibilidade::IlegivelSemChave { seladas: 318 });
        assert!(
            v.e_falha(),
            "um restore ilegível reportou-se como aceitável"
        );
        assert!(!v.verificou());
    }

    /// Uma instalação nova sem chave e sem estado não falha.
    ///
    /// O contrário do teste acima, e sem ele o comando recusaria arrancar em
    /// toda a instalação que ainda não ligou correio nenhum.
    #[test]
    fn instalacao_nova_sem_estado_nao_falha() {
        let v = legibilidade(false, 0, 0);
        assert_eq!(v, Legibilidade::NadaParaLer);
        assert!(!v.e_falha());
        // E também não conta como verificação: não se observou nada.
        assert!(!v.verificou(), "«nada para ler» passou por «está tudo bem»");
    }

    /// Ter chave e nenhum estado não é prova de nada.
    #[test]
    fn chave_sem_estado_nao_prova_nada() {
        let v = legibilidade(true, 0, 0);
        assert_eq!(v, Legibilidade::ChaveSemEstado);
        assert!(!v.e_falha());
        assert!(
            !v.verificou(),
            "uma chave por usar passou por chave provada"
        );
    }

    /// Tudo aberto é a única coisa que conta como verificado.
    #[test]
    fn tudo_aberto_e_o_unico_verde() {
        let v = legibilidade(true, 318, 0);
        assert_eq!(v, Legibilidade::Legivel { abriram: 318 });
        assert!(v.verificou());
        assert!(!v.e_falha());
    }

    /// Uma linha que não abre chega para falhar.
    ///
    /// Não há maioria nem tolerância: uma credencial que não abre é um membro
    /// que não recebe correio, e a instituição não sabe qual.
    #[test]
    fn uma_linha_que_nao_abre_chega_para_falhar() {
        let v = legibilidade(true, 318, 1);
        assert_eq!(
            v,
            Legibilidade::Ilegivel {
                recusadas: 1,
                total: 318
            }
        );
        assert!(v.e_falha());
        assert!(!v.verificou());
    }

    /// O que viaja por canal próprio é exactamente o que interpreta história.
    #[test]
    fn viaja_o_que_interpreta_historia() {
        let viajam: Vec<&str> = viaja_por_canal_proprio()
            .into_iter()
            .map(|m| m.variavel)
            .collect();
        assert_eq!(
            viajam,
            vec!["OCINYE_MAIL_KEY"],
            "o conjunto do material durável mudou. Isto não é um erro por si: é \
             uma alteração ao que uma migração tem de transportar por canal \
             seguro, e tem de ser deliberada"
        );
    }
}
