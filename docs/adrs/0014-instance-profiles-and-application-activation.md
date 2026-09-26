# ADR-0014 — Perfis de Instância e activação de aplicações

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** [ADR-0013](0013-general-purpose-os-instance-and-node.md)
- **Data:** 2026-09-26

## Context

Com a [ADR-0013](0013-general-purpose-os-instance-and-node.md) o Ocinye OS deixou
de assumir a organização de ninguém. Continua a assumir o **tipo** de
organização: uma de investigação.

- As quatro unidades da Ocinye são semeadas em toda a Instância que nasce.
- Todas as aplicações existem sempre: uma empresa vê Ideias, Bibliografia,
  Datasets e Computação, e não as consegue retirar sem mudar código.
- A relevância de módulo deriva de papéis de investigação. Ficheiros só aparece
  a `ResearchLead`, `ResearchMember`, `UnitManager` e `OrganisationAdmin`: um
  membro `Collaborator` de uma empresa nunca veria **os seus próprios**
  ficheiros, e o administrador da plataforma também não.

O programa de generalização pede que isto passe a ser configuração: perfis de
investigação, empresa, pessoal e educação sobre o mesmo sistema, e aplicações
opcionais que se desactivam sem danificar o Core.

## Decision

**1. Um Perfil é um atributo da Instância.** Quatro, fechados:
`research`, `business`, `personal`, `education`. Vive em
`instance_identity.profile`. Muda-se depois da instalação, por quem tem
`organisation.manage`, e mudar nunca apaga dados.

> **Um perfil não é autorização.** Decide que aplicações começam activas, que
> atalhos o membro encontra fixados, e se a estrutura inicial de unidades é
> semeada. Quem pode fazer o quê continua a ser o RBAC.

**2. Uma Instância nova escolhe o perfil, e não há perfil por omissão.** Um
valor por omissão seria outra vez o produto a assumir que tipo de organização é.
A instalação existente — a Ocinye — é `research`, e comporta-se exactamente como
antes.

**3. As aplicações têm um identificador conhecido do Core e uma classe.** O
catálogo de identificadores passa a `ocinye-contracts` (`ApplicationId`), para
que o Core valide o que guarda; o registo do Workspace
([§45-A](../applications/README.md)) apoia-se nele. Duas classes:

| Classe | Aplicações | Desactivável |
|---|---|---|
| **Essencial** | Home, O Meu Trabalho, Ficheiros, Meus Recursos, Administração, Definições, Ajuda | não — sem elas não há sistema operativo |
| **Opcional** | todas as outras | sim, por Instância |

**4. A activação é por Instância, e o perfil dá a predefinição.** Uma aplicação
opcional está activa se a Instância o disse explicitamente
(`instance_applications`), ou, sem decisão explícita, se o perfil a traz. Mudar
de perfil muda as predefinições das aplicações sem decisão explícita, e nada
mais.

| Aplicação | research | business | education | personal |
|---|:-:|:-:|:-:|:-:|
| Notas, Calendário, Correio, Prompt | ✓ | ✓ | ✓ | ✓ |
| Mensagens, Actividade, Audit Log | ✓ | ✓ | ✓ | |
| Unidades, Projectos | ✓ | ✓ | ✓ | |
| Conhecimento, Bibliografia | ✓ | | ✓ | |
| Ideias, Datasets | ✓ | | | |
| Ocinye AI, Agentes, Computação | ✓ | | | |

**5. Desactivar uma aplicação esconde-a, e não a apaga.** Sai do lançador, da
barra, da paleta e do «+ Criar»; a sua rota diz que a aplicação não está activa
nesta Instância. Os dados ficam, e reactivar devolve-a tal como estava.
`Desactivar ≠ desinstalar`, como `desafixar ≠ desinstalar`. A recusa da API de
uma aplicação desactivada é da fronteira Core/aplicações (Parte 3).

**6. Ficheiros é relevante a todo o membro interno.** O armazenamento pessoal é
uma capacidade do sistema operativo, não da investigação. A relevância continua a
não conceder autoridade: o que cada pessoa vê dentro de Ficheiros é o que já via.

**7. As unidades iniciais só são semeadas no perfil `research`.** Noutros perfis
a Instância nasce sem estrutura, e quem a administra cria a sua.

## Alternatives

**Perfis como distribuições (binários diferentes).** É exactamente o fork que o
programa proíbe.

**Relevância de módulo por perfil.** Tornaria a política de navegação
dependente de configuração, e dois sítios passariam a decidir a mesma coisa.
Ficheiros deixa de depender de investigação em todos os perfis; os módulos de
investigação continuam como estavam.

**Renomear papéis e unidades por perfil («departamento» em vez de «unidade»).**
Terminologia configurável é útil e é outra decisão: mexe no catálogo i18n e na
semântica dos papéis. Fica fora desta ADR.

## Consequences

- O Workspace passa a perguntar ao Core que aplicações a Instância tem activas;
  a visibilidade de uma aplicação é `activa ∧ autorizada ∧ relevante`.
- Um `Collaborator`, um `PlatformAdmin` sem papel de investigação e um
  `Auditor` passam a ver Ficheiros no lançador. Nenhum passa a ler nada que não
  lesse: o conteúdo continua governado pela posse e pela autorização.
- `bootstrap-admin` exige `--profile` para criar uma Instância.
