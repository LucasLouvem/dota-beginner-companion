# Plano 001 — MVP do Dota Beginner Companion

**Status:** Em andamento
**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24
**Documentos relacionados:** [ARCHITECTURE](../docs/ARCHITECTURE.md) · [TESTING](../docs/TESTING.md) · [ERRORS](../docs/ERRORS.md) · [CODE_COMMENTS](../docs/CODE_COMMENTS.md) · [DOCUMENTATION](../docs/DOCUMENTATION.md) · [COMMITS](../docs/COMMITS.md)

**Objetivo:** entregar um aplicativo desktop para Linux (Arch + KDE Plasma) que acompanha jogadores iniciantes de Dota 2 antes e depois da partida. Ele recebe o estado do jogo pelo GSI e mostra sugestões estatísticas de itens e habilidades a partir de dados públicos. O objetivo é ajudar quem está começando a entender as escolhas comuns, sem interferir no jogo.

**Fora de escopo:**

- Overlay ou qualquer elemento desenhado sobre a janela do Dota 2.
- Leitura de memória, injeção de DLL, hooks ou alteração do processo do jogo.
- Automação de cliques, compras, escolha de habilidades ou comandos.
- Exibir informações ocultas dos inimigos ou dados de outros jogadores além dos que o próprio jogo mostra.
- Servidores próprios ou envio de dados do usuário a terceiros sem consentimento.
- Microserviços ou processos separados.
- Valve Web API nesta versão.

**Legenda:** `✓ Concluída` · `-> Atual` · sem marcador = pendente

---

## Fase 0 — Base ✓ Concluída

**Escopo:** toolchain do projeto, repositório e arquivos básicos de um projeto open source.

**Decisões tomadas:**

- Rust gerenciado pelo asdf via `.tool-versions` local, porque o mantenedor isola as toolchains por projeto.
- `rust-toolchain.toml` com a mesma versão, para quem usa rustup sem asdf.
- Licença GPL-3.0-or-later, para que forks continuem abertos.
- Interface em PT-BR com gettext, para permitir outros idiomas depois.
- Mensagens de commit em Conventional Commits, em inglês ([COMMITS](../docs/COMMITS.md)).
- Verificação de formatação e escrita com dprint (Markdown/TOML), rumdl (lint de Markdown) e typos (ortografia do código), para manter documentação e código padronizados ([TESTING](../docs/TESTING.md#comandos)).
- Arquitetura: monólito modular orientado a eventos ([ARCHITECTURE](../docs/ARCHITECTURE.md)).

**Resultados mínimos esperados:**

- `asdf install` instala a versão de Rust declarada, e `cargo --version` responde dentro do repositório.
- `cargo build` compila sem erros.
- Existem `LICENSE` (GPL-3.0-or-later), `README.md`, `CONTRIBUTING.md`, `.env.example` e `.gitignore`. O `.gitignore` ignora `target/` e `.env`.
- `cargo fmt --check` e `cargo clippy --all-targets -- -D warnings` passam.

## Fase 1 — Janela, diagnóstico e coletor GSI -> Atual

**Depende de:** Fase 0

**Escopo:** janela principal com abas, tela de Diagnóstico, servidor GSI local, instalação do arquivo `.cfg` do GSI, localização do Dota e das bibliotecas Steam, e visualização do JSON recebido.

**Decisões tomadas:**

- Servidor GSI em `127.0.0.1`, com token e limite de corpo ([ARCHITECTURE](../docs/ARCHITECTURE.md#segurança-do-servidor-gsi)).
- Porta padrão 53000, configurável, para evitar portas comuns de desenvolvimento.
- O `.cfg` não pede os blocos `draft` e `wearables`, porque o draft pode trazer escolhas de outros jogadores.
- Arquivo `.cfg` existente e diferente só é substituído com confirmação e backup.
- "Dota aberto" é detectado pelo nome do processo em `/proc/*/comm`, sem acessar o processo.
- O evento do botão "Testar conexão" é contado separado dos eventos do Dota, para não aparecer como conexão com o jogo.

**Resultados mínimos esperados:**

- `cargo test` passa, com casos de sucesso e de erro para: parser VDF, localizador Steam, instalação do `.cfg` (criar, inalterado, pedir confirmação, backup, falha de permissão), servidor GSI (token válido, token inválido, JSON inválido, corpo acima de 1 MB, bind em loopback) e teste de conexão.
- `cargo run` abre a janela. O Diagnóstico mostra o Dota localizado, o estado do arquivo GSI, o servidor ouvindo, o número de eventos e o horário do último evento.
- Um POST com token correto para `127.0.0.1:<porta>` aparece no Diagnóstico com o JSON sem o bloco `auth`. Com token errado, o contador de recusadas aumenta e a resposta é 401.
- `ss -ltn` mostra a porta do GSI apenas em `127.0.0.1`.
- Com o `.cfg` instalado e a opção `-gamestateintegration`, uma partida contra bots gera eventos do Dota no Diagnóstico.

**Em aberto:**

- Os resultados mínimos foram verificados com o jogo real: janela, POST com token certo e errado, `ss -ltn` e partidas contra bots.
- O código ainda não segue [CODE_COMMENTS](../docs/CODE_COMMENTS.md) em dois pontos que citam o futuro: o comentário de `application/ports.rs` menciona providers, e as abas ainda vazias mostram "Disponível na Fase N do projeto.".
- O uso de `unsafe` para `setlocale` precisa de aprovação.
- Faltam `scripts/dev.sh` e `scripts/send_test_event.sh`?

## Fase 2 — Normalizador, herói e storage

**Depende de:** Fase 1

**Escopo:** converter o JSON do GSI em estado de jogo do domínio, emitir eventos (herói selecionado, início e fim da partida, itens e habilidades), criar a persistência local em SQLite (cache e configurações) e a aba Herói. Inclui o botão manual "Partida encontrada".

**Decisões tomadas:**

- "Partida encontrada" é tratada como experimental: é inferida pela entrada na seleção de herói e também tem botão manual, porque o GSI não expõe esse evento diretamente.
- Todo evento e indicador informa se é confirmado pelo GSI ou inferido, para nunca apresentar inferência como fato.
- Os campos do payload GSI são opcionais, porque variam por modo de jogo.
- Persistência local em SQLite na camada `storage` ([ARCHITECTURE](../docs/ARCHITECTURE.md#decisões-aprovadas-e-não-implementadas)).

**Resultados mínimos esperados:**

- Testes com sequências de payloads (fixtures) geram os eventos esperados, na ordem certa. Payloads incompletos ou fora de ordem não geram eventos falsos.
- Testes do storage cobrem gravação, leitura, expiração por TTL, dado vencido marcado como antigo e limpeza do cache.
- Em uma partida real ou contra bots, a aba Herói mostra o herói escolhido com nome, atributo, funções, posições, dificuldade e habilidades.
- O botão "Partida encontrada" registra o evento como manual.

**Em aberto:**

- Qual é a fonte dos dados estáticos dos heróis (atributos, funções, dificuldade): constantes embutidas, OpenDota ou ambas?

## Fase 3 — Providers e recomendações

**Depende de:** Fase 2

**Escopo:** integração com o OpenDota (constantes, patch, partidas profissionais recentes, detalhes de partidas), cache com fallback e cálculo de frequência de itens, momento médio de compra, taxa de vitória e ordem de habilidades. Abas Itens, Habilidades e Dados profissionais. Provider STRATZ opcional, ativo só com token configurado.

**Decisões tomadas:**

- Interface de provider com fallback para cache vencido e dados embutidos, para nunca depender só de uma API externa ([ARCHITECTURE](../docs/ARCHITECTURE.md#decisões-aprovadas-e-não-implementadas)).
- Recomendações abaixo da amostra mínima (padrão 10 partidas, configurável) aparecem como "amostra insuficiente", porque uma única partida não serve de base.
- Toda recomendação mostra fonte, patch, data de atualização e quantidade de partidas, e é apresentada como sugestão estatística.
- Chaves de API ficam em `.env` ou na configuração local, nunca no código.

**Resultados mínimos esperados:**

- Testes com HTTP simulado cobrem resposta válida, erro de rede, limite de requisições (429) e resposta malformada, sem acessar a internet.
- Testes de agregação cobrem frequência, momento médio, taxa de vitória com e sem amostra suficiente e ordem de habilidades mais comum.
- Com a rede desligada e cache existente, as abas mostram os dados marcados como antigos. Sem cache, mostram os dados embutidos.
- A aba Itens mostra, para o herói atual, item, frequência, taxa de vitória (quando houver), momento médio, contexto de uso, partidas analisadas, patch e fonte.

**Em aberto:**

- Os termos de uso da STRATZ permitem esse uso?
- Como filtrar por posição quando a partida não informa a lane?

## Fase 4 — Pós-partida, histórico, notificações e empacotamento

**Depende de:** Fase 3

**Escopo:** resumo pós-partida (resultado, herói, duração, KDA, last hits, denies, GPM, XPM, itens, habilidades, comparação com partidas semelhantes, três pontos positivos e três a melhorar), busca posterior da partida no OpenDota, aba Histórico, notificação de "resumo pronto" e pacote para Arch Linux.

**Decisões tomadas:**

- Notificações desligadas por padrão e opt-in, para não serem intrusivas ([ARCHITECTURE](../docs/ARCHITECTURE.md#decisões-aprovadas-e-não-implementadas)).
- Histórico de partidas persistido via `storage` ([ARCHITECTURE](../docs/ARCHITECTURE.md#decisões-aprovadas-e-não-implementadas)).
- Buscar o histórico pelo SteamID só com consentimento do usuário.

**Resultados mínimos esperados:**

- Testes da análise pós-partida cobrem vitória, derrota, dados completos e dados faltando, e geram sempre três pontos positivos e três a melhorar.
- Ao fim de uma partida real ou contra bots, o resumo aparece com os dados do GSI. A busca pelo ID da partida completa o que faltar.
- A aba Histórico lista as partidas salvas depois de reiniciar o app.
- `makepkg` gera um pacote que, depois de instalado, abre o app pelo menu do KDE.

**Em aberto:**

- Com que critérios as partidas são consideradas "semelhantes" na comparação?

---

## Riscos

| Risco                                                        | Impacto                                           | Mitigação                                                                                                    |
| ------------------------------------------------------------ | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| O Dota exige a opção `-gamestateintegration`                 | Nenhum evento chega e o app parece quebrado       | O Diagnóstico explica a opção e mostra se algum evento foi recebido                                          |
| O GSI não envia dados no menu principal                      | "Conectado" pode ser confundido com "jogo aberto" | Mostrar separadamente processo aberto e eventos recebidos                                                    |
| A Valve muda o formato do payload GSI                        | O parser deixa de reconhecer campos               | Campos opcionais e fixtures de payloads reais nos testes                                                     |
| Steam Flatpak ou Proton usam caminhos diferentes             | Dota não localizado                               | Busca nos caminhos Flatpak, bibliotecas extras configuráveis e lista dos caminhos verificados no Diagnóstico |
| Porta do GSI já em uso                                       | Servidor não inicia                               | Erro de bind exibido com a porta; porta configurável                                                         |
| Limite de requisições ou indisponibilidade do OpenDota       | Recomendações sem dados                           | Rate limit local, cache com TTL, dados vencidos marcados e dados embutidos                                   |
| Amostra pequena de partidas profissionais por herói ou patch | Sugestões pouco confiáveis                        | Amostra mínima configurável e quantidade de partidas sempre visível                                          |
| Termos de uso de APIs de terceiros                           | Provider pode precisar ser removido               | Providers desacoplados e STRATZ opcional                                                                     |

## Mudanças no plano

- 2026-10-24 — Plano criado a partir das fases definidas para o projeto.
- 2026-10-24 — Decisões de storage, providers, notificações e histórico passam a referenciar a seção de decisões aprovadas do ARCHITECTURE.
- 2026-10-24 — Fase 0 concluída: resultados mínimos verificados.
- 2026-10-24 — Fase 1 marcada como atual.
- 2026-10-24 — Verificação de formatação e escrita (dprint, rumdl, typos) adicionada às decisões da Fase 0.
- 2026-10-24 — Fase 1: o código passa a seguir o contrato central de diagnóstico ([ERRORS](../docs/ERRORS.md)) e a organização de testes ([TESTING](../docs/TESTING.md)), com testes unitários em `tests/unit/`, `src/lib.rs` e testes de integração. O relatório de diagnóstico passa do `domain` para a `application`.
- 2026-10-24 — Fase 1: falhas antes ignoradas passam a ser tratadas. O `.env` é lido sem alterar o ambiente e impede a inicialização se for inválido; as tarefas do núcleo e o gravador de eventos informam interrupções; canais fechados geram 503 no servidor e aviso na interface; tradução e limpeza de temporários registram aviso no log.
