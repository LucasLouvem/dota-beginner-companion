# Plano 001 — MVP do Dota Beginner Companion

**Status:** Em andamento
**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-27
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

## Fase 1 — Janela, diagnóstico e coletor GSI ✓ Concluída

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

**Verificações e decisões de fechamento:**

- Os resultados mínimos foram verificados anteriormente com o jogo real: janela, POST com token certo e errado, `ss -ltn` e partidas contra bots.
- O uso de `unsafe` para `setlocale` foi aprovado pelo mantenedor em 2026-10-24, limitado à inicialização antes de criar threads. A condição está documentada no código.
- Comentários e abas não citam funcionalidades futuras nem números de fases; abas sem implementação informam a indisponibilidade.
- `scripts/send_test_event.sh` envia um evento sintético com a configuração do app. `scripts/dev.sh` foi dispensado: `cargo run` já cobre a execução de desenvolvimento.
- Pânicos recebem saída própria no terminal, sem payload, nome da thread ou backtrace; diagnósticos de tarefas interrompidas preservam seu comportamento.

**Validação de fechamento:**

- Janela aberta e inspecionada visualmente; as seis abas sem conteúdo exibem a mensagem de indisponibilidade, também conferida pela acessibilidade.
- Envio manual pelo script confirmado no aplicativo aberto. Testes de subprocesso verificam as prioridades da configuração, recusas e proteção da saída de pânicos.
- `scripts/check.sh` passa com 133 testes unitários e 12 de integração. A Fase 2 permanece pendente de confirmação do mantenedor.

**Observações de partidas reais:**

Relatadas em duas partidas contra bots (Dragon Knight e Zeus), com 849 eventos, no registro local `logs/PROMPT-NOTEBOOK.md`. São comportamentos observados, não garantias da API da Valve. O log contém dados pessoais e não deve ser versionado; fixtures precisam ser anonimizadas.

- A sequência observada foi menu (sem `map`), seleção de herói, estratégia, apresentação dos times, carregamento, pré-jogo e partida em andamento.
- O menu gera eventos com `provider` e `player`, sem `map`.
- Antes da escolha, `hero` aparece com `id: 0`; nome e ID do herói aparecem depois.
- Compras na estratégia reduzem o ouro, mas os itens só aparecem no inventário a partir do pré-jogo.
- `map.clock_time` retrocede entre estados; a ordenação deve considerar a chegada dos eventos, não esse relógio.
- Não foi observado campo de rota, lane ou posição de jogo. Coordenadas do herói não confirmam uma função na equipe.
- `map.matchid` é `"0"` contra bots durante a partida. Uma partida completa mostrou que o ID real aparece no `POST_GAME` (ver Fase 2).
- O inventário é o estado atual, sem histórico completo de compras. Comparar eventos pode indicar alterações, sem garantir a causa de cada alteração.
- O log não cobre `POST_GAME` nem `win_team`; o fim de partida foi capturado depois, na verificação da Fase 2.

## Fase 2 — Normalizador, herói e storage ✓ Concluída

**Depende de:** Fase 1

**Escopo:** converter o JSON do GSI em estado de jogo do domínio, emitir eventos (herói selecionado, início e fim da partida, itens e habilidades), criar a persistência local em SQLite (cache e configurações) e a aba Herói. Inclui o botão manual "Partida encontrada".

**Decisões tomadas:**

- "Partida encontrada" é tratada como experimental: é inferida pela entrada na seleção de herói e também tem botão manual, porque o GSI não expõe esse evento diretamente.
- Todo evento e indicador informa se é confirmado pelo GSI ou inferido, para nunca apresentar inferência como fato.
- Os campos do payload GSI são opcionais, porque variam por modo de jogo.
- Persistência local em SQLite na camada `storage` ([ARCHITECTURE](../docs/ARCHITECTURE.md#decisões-aprovadas-e-não-implementadas)), com a crate `rusqlite` e a feature `bundled`, para não depender da versão do SQLite do sistema.
- Dados estáticos dos heróis (nome, atributo primário, tipo de ataque, funções e habilidades) vêm de um recorte do [odota/dotaconstants](https://github.com/odota/dotaconstants) (MIT) embutido no binário, com fonte, commit e data registrados. Funciona sem rede e é o dado embutido previsto como último fallback dos providers. A atualização pelo OpenDota fica para a Fase 3.
- Posição (1 a 5) e dificuldade não fazem parte desta fase: nenhuma fonte verificada traz esses campos (o dotaconstants não tem, o GSI não envia e o log não tem lane), e deduzi-los das funções seria inferência apresentada como dado.
- Sem Steam e Dota no ambiente de desenvolvimento atual, o comportamento com o jogo é validado reenviando ao aplicativo os eventos gravados em `logs/gsi.jsonl`.
- A aba Herói mantém o último herói e os dados da última partida jogada (resultado, duração, KDA, farm, mortes, itens) depois que a partida termina, até a próxima partida ser encontrada, de forma automática ou manual. A apresentação visual fica para depois. Decidido pelo mantenedor depois da partida real, em que o card ficava vazio ao voltar ao menu.
- A configuração continua no `config.toml`, com o token e permissão `0600`; o SQLite fica para cache e, mais tarde, histórico.
- O botão manual "Partida encontrada" reinicia a linha do tempo, como a detecção automática.
- O espaço do TP (`teleport0`) não gera eventos na linha do tempo, porque o TP é usado e reposto o tempo todo; ele continua no inventário exibido.
- Nomes de exibição e receitas dos itens vêm de um recorte de `build/items.json` do mesmo commit do dotaconstants, com o comando do recorte registrado em `assets/dotaconstants/SOURCE.md`. A montagem de um item é inferida quando ele entra na mesma mudança em que saem componentes da receita.

**Resultados mínimos esperados:**

- Testes com sequências de payloads (fixtures) geram os eventos esperados, na ordem certa. Payloads incompletos ou fora de ordem não geram eventos falsos.
- Testes do storage cobrem gravação, leitura, expiração por TTL, dado vencido marcado como antigo e limpeza do cache.
- Com os eventos gravados reenviados ao aplicativo, a aba Herói mostra o herói escolhido com nome, atributo, tipo de ataque, funções e habilidades.
- O botão "Partida encontrada" registra o evento como manual.

**Verificações:**

- Fixtures anonimizadas de duas partidas reais cobrem menu sem `map`, herói com `id: 0`, compras na estratégia sem itens visíveis, relógio que retrocede, inventário a partir do pré-jogo, nível de habilidade, item movido do stash e morte. A lista de eventos esperada foi escrita à mão e confere com a saída do normalizador e do `MatchTracker`.
- O fim de partida é coberto por um recorte anonimizado da partida real (montagem do Satanic, pós-jogo com o ID ainda `"0"`, pós-jogo com o ID e volta ao menu); o ID real foi trocado por um fictício. A fixture sintética foi removida.
- O storage tem testes de gravação, leitura, expiração por TTL com dado vencido marcado como antigo, limpeza, reabertura, banco corrompido, pasta sem permissão e schema mais novo.
- Com o log real (849 eventos) reenviado ao app aberto, todos foram aceitos. A aba Herói mostrou Zeus com atributo, ataque e funções dos dados embutidos, as habilidades com os níveis do GSI e a linha do tempo na ordem certa. O botão "Partida encontrada" registrou o evento como manual. A conferência foi feita pela acessibilidade (AT-SPI) e por captura de tela.
- Partida real completa contra bots com o app aberto (Dragon Knight, vitória, 2.425 eventos, do menu ao pós-jogo), conferida pela acessibilidade contra o log gravado:
  - a linha do tempo mostrou "Partida encontrada" (inferido), herói escolhido, início e fim da partida com vitória (confirmados pelo GSI) nos mesmos horários de chegada do log;
  - as 15 subidas de habilidade e as 33 alterações de inventário do log apareceram, com a origem correta;
  - o Diagnóstico contou 2.425 eventos recebidos e gravados, igual ao arquivo.
- Observado nessa partida: `map.matchid` passa de `"0"` para o ID real no `POST_GAME`; `win_team` e `player.team_name` permitem o resultado; o estado de carregamento do mapa não apareceu, então não é garantido.
- O log continuou gravando mais três partidas completas (Dragon Knight, Axe e Dawnbreaker; 10.892 eventos no total). Nelas apareceu `DOTA_GAMERULES_STATE_WAIT_FOR_PLAYERS_TO_LOAD` antes da seleção de heróis, e o app aberto deixou de registrar partida encontrada, início e fim a partir da segunda partida. A detecção foi corrigida para considerar essa fase, com teste da sequência.
- O primeiro evento de `POST_GAME` ainda traz o ID `"0"`; o ID real vem no evento seguinte. O resumo da última partida é refeito a cada evento do pós-jogo, com teste.
- O log completo passado pelo normalizador, pelo `MatchTracker` com as receitas embutidas e pelo estado da partida (sem a interface) deu 4 partidas encontradas, iniciadas e terminadas, com os resultados do log, 19 montagens de itens, nenhum evento de TP, e a aba Herói no fim com a última partida (Dawnbreaker, derrota, 17:40, 2/7/0, 34/0, GPM 481, XPM 1350, nível 21 e o ID da partida).
- A interface passou a coalescer os estados (no máximo um aviso pendente para a thread do GTK) e a atualizar só as linhas que mudaram nas abas Herói e Diagnóstico. Com 3.000 eventos reenviados a 5 ms por evento, em build de depuração:
  - a atualização da aba Herói caiu de 14,2 ms para 0,34 ms (mediana);
  - a fila de estados, que antes levava cerca de 8 s para esvaziar depois do reenvio, deixou de se acumular;
  - a thread do GTK caiu de 99% para 78% de um núcleo; o restante é layout e desenho do GTK.
- Leitura pela acessibilidade (AT-SPI), depois do reenvio do log completo com essas mudanças:
  - a aba Herói mostrou a última partida (Dawnbreaker, derrota, 17:40, 2/7/0, GPM 481, XPM 1350, nível 21) e a linha do tempo em ordem, com o evento mais recente em cima;
  - a aba Diagnóstico mostrou 3.000 eventos recebidos e gravados;
  - o botão "Testar conexão" acrescentou a linha de sucesso sem alterar a contagem de eventos do Dota.
- Diagnósticos concorrentes (automático, manual e o feito depois da instalação) passaram a ser rodadas numeradas numa só sequência, e só a mais recente publica. Antes, uma rodada que lia os arquivos antes da instalação e terminava depois dela substituía o diagnóstico novo pelo antigo. Testes determinísticos do núcleo cobrem:
  - diagnóstico antigo terminando depois da instalação;
  - interrupção antiga;
  - instalação ultrapassada por uma rodada mais nova;
  - limpeza da interrupção pela rodada seguinte.
- O teste de integração da instalação, que falhava de forma intermitente por essa corrida (6 falhas em 400 execuções paralelas com a máquina carregada), passou a esperar o primeiro diagnóstico e não falhou em 400 execuções nas mesmas condições.

**Estado em 2026-10-26:**

Feito (as issues #56 a #59 e #62 a #63 estão no código, sem commit ainda neste momento):

- Builds reais em vez de agregadas (#56), ordem de compra por fase e o aprimoramento neutro separado (#57 e #58).
- Página única da aba Herói (#59): lista de vitórias, cabeçalho com retrato (#62), grade da ordem de habilidades com ícones (#62), ícones dos itens, progresso da partida, K/D/A na lista (banco v4) e comparação "Você × build" com as vitórias profissionais (#63, banco v5).
- Layout compacto e responsivo em duas composições (a janela não encolhe abaixo de 790 px) e abas no topo da janela; as abas Itens, Habilidades e Dados profissionais saíram.
- Estilo próprio do aplicativo (`presentation::style`), com prioridade do usuário e classes `dota-`.
- Limpeza do que a página deixou de usar (#61, parte): o conteúdo antigo da aba Herói, a alteração do modo depois da partida encontrada e a seleção antiga; a descrição da linha do tempo foi para `presentation::pages::timeline`, sem uso até a aba Histórico (Fase 4). Os testes seguiram: 535 unitários e 18 de integração.

Falta fazer (Fase 3):

- Conferir numa partida real, com o jogo aberto, a página nova ao vivo (#27, cobrindo também a tela cheia e a janela no tamanho mínimo).
- Terminar os testes manuais da #59 (teclado, estados sem herói, Turbo, Contra bots, sem builds e rede desligada).
- Decidir a #55 (usar as partidas públicas contra bots no modo Contra bots), incluindo investigar se algum campo real do GSI identifica partidas contra bots.
- Conferir a rede desligada no app (o fallback é coberto por testes, mas não foi visto na tela).
- Investigar o aviso do GTK sobre a altura mínima de uma caixa da coluna principal, que pode estar ligado ao antigo travamento em janelas pequenas.
- Decidir o que fazer com a coleta de popularidade por fase, que o app ainda busca no OpenDota mas a página deixou de mostrar (uma chamada por consulta; #61).
- Classificação de perfil das builds (#60): aguardando uma fonte de dados.
- Marcar a Fase 3 como concluída só com a confirmação do mantenedor.

**Em aberto:**

- Conferência numa partida real com o jogo aberto, com captura de tela (#27).
- Conferência manual com a rede desligada.
- Usar as partidas públicas contra bots (`lobby_type` 4) como fonte do modo Contra bots (#55).
- As vitórias públicas de Escolha Livre aparecem devagar, porque a maioria das partidas públicas é turbo, e o orçamento diário da coleta pode acabar cedo.
- Guardar o último modo de jogo confirmado entre execuções do app, junto das configurações no SQLite.

## Fase 4 — Pós-partida, histórico, notificações e empacotamento

**Depende de:** Fase 3

**Escopo:** resumo pós-partida (resultado, herói, duração, KDA, last hits, denies, GPM, XPM, itens, habilidades, comparação com partidas semelhantes, três pontos positivos e três a melhorar), busca posterior da partida no OpenDota, aba Histórico, notificação de "resumo pronto" e pacote para Arch Linux.

**Decisões tomadas:**

- Notificações desligadas por padrão e opt-in, para não serem intrusivas ([ARCHITECTURE](../docs/ARCHITECTURE.md#decisões-aprovadas-e-não-implementadas)).
- Histórico de partidas persistido via `storage` ([ARCHITECTURE](../docs/ARCHITECTURE.md#decisões-aprovadas-e-não-implementadas)).
- Na aba Histórico, cada partida abre uma tela com os dados dela; as últimas partidas jogadas ficam listadas. A apresentação visual é tratada depois do conteúdo.
- Buscar o histórico pelo SteamID só com consentimento do usuário.
- Cada evento da linha do tempo passa a guardar também o relógio da partida (`clock_time`, que o normalizador já lê), para mostrar em que minuto de jogo cada coisa aconteceu, como a compra de cada item. O horário de chegada continua. Fica nesta fase, e não na Fase 3, porque a pergunta "em que minuto comprei o item X" é respondida no resumo pós-partida e no histórico; a Fase 3 trata de dados externos agregados, e a comparação com partidas semelhantes, que usaria esse minuto, também é desta fase.

**Resultados mínimos esperados:**

- Testes da análise pós-partida cobrem vitória, derrota, dados completos e dados faltando, e geram sempre três pontos positivos e três a melhorar.
- Ao fim de uma partida real ou contra bots, o resumo aparece com os dados do GSI. A busca pelo ID da partida completa o que faltar.
- A aba Histórico lista as partidas salvas depois de reiniciar o app.
- `makepkg` gera um pacote que, depois de instalado, abre o app pelo menu do KDE.

**Em aberto:**

- Com que critérios as partidas são consideradas "semelhantes" na comparação?
- O ID da partida só chega no `POST_GAME`: definir o que guardar quando o app fechar antes ou esse evento não chegar.
- Decidir se o histórico e o resumo com dados do GSI saem antes da Fase 3, já que não dependem de dados externos.

---

## Riscos

| Risco                                                        | Impacto                                               | Mitigação                                                                                                    |
| ------------------------------------------------------------ | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| O Dota exige a opção `-gamestateintegration`                 | Nenhum evento chega e o app parece quebrado           | O Diagnóstico explica a opção e mostra se algum evento foi recebido                                          |
| Eventos no menu não incluem `map`                            | Evento recebido pode ser confundido com partida ativa | Separar processo aberto, eventos recebidos e presença de dados de partida                                    |
| A Valve muda o formato do payload GSI                        | O parser deixa de reconhecer campos                   | Campos opcionais e fixtures de payloads reais nos testes                                                     |
| Steam Flatpak ou Proton usam caminhos diferentes             | Dota não localizado                                   | Busca nos caminhos Flatpak, bibliotecas extras configuráveis e lista dos caminhos verificados no Diagnóstico |
| Porta do GSI já em uso                                       | Servidor não inicia                                   | Erro de bind exibido com a porta; porta configurável                                                         |
| Limite de requisições ou indisponibilidade do OpenDota       | Recomendações sem dados                               | Rate limit local, cache com TTL, dados vencidos marcados e dados embutidos                                   |
| Amostra pequena de partidas profissionais por herói ou patch | Sugestões pouco confiáveis                            | Amostra mínima configurável e quantidade de partidas sempre visível                                          |
| Termos de uso de APIs de terceiros                           | Provider pode precisar ser removido                   | Providers desacoplados e STRATZ opcional                                                                     |
| ID real da partida só chega no `POST_GAME`                   | Sem ID, não há busca posterior no OpenDota            | Guardar o resumo local com os dados do GSI e buscar pelo ID só quando ele existir                            |

## Mudanças no plano

- 2026-10-24 — Plano criado a partir das fases definidas para o projeto.
- 2026-10-24 — Decisões de storage, providers, notificações e histórico passam a referenciar a seção de decisões aprovadas do ARCHITECTURE.
- 2026-10-24 — Fase 0 concluída: resultados mínimos verificados.
- 2026-10-24 — Fase 1 marcada como atual.
- 2026-10-24 — Verificação de formatação e escrita (dprint, rumdl, typos) adicionada às decisões da Fase 0.
- 2026-10-24 — Fase 1: o código passa a seguir o contrato central de diagnóstico ([ERRORS](../docs/ERRORS.md)) e a organização de testes ([TESTING](../docs/TESTING.md)), com testes unitários em `tests/unit/`, `src/lib.rs` e testes de integração. O relatório de diagnóstico passa do `domain` para a `application`.
- 2026-10-24 — Fase 1: falhas antes ignoradas passam a ser tratadas. O `.env` é lido sem alterar o ambiente e impede a inicialização se for inválido; as tarefas do núcleo e o gravador de eventos informam interrupções; canais fechados geram 503 no servidor e aviso na interface; tradução e limpeza de temporários registram aviso no log.
- 2026-10-24 — Fechamento da Fase 1: `setlocale` aprovado antes das threads, referências futuras removidas das abas e comentários, envio sintético por script e proteção da saída de pânicos. Achados relatados de partidas reais registrados com impactos nas Fases 2 e 4. Conferência visual e envio manual realizados; Fase 1 concluída, com a Fase 2 pendente de confirmação.
- 2026-10-24 — Fase 2 marcada como atual. Decididos: dados dos heróis embutidos a partir do dotaconstants, posição e dificuldade fora da fase por falta de fonte, storage com `rusqlite` (`bundled`) e validação com o log reenviado ao app enquanto não há Dota no ambiente.
- 2026-10-24 — Fase 2 implementada na branch `feat/phase-2-game-state-hero-storage`: normalizador do GSI, eventos de partida com origem, linha do tempo, "Partida encontrada" manual, aba Herói com dados embutidos do dotaconstants, cache em SQLite e ferramenta de reenvio do log. Aguardando revisão do mantenedor antes de marcar a fase como concluída.
- 2026-10-24 — Fase 2 mesclada na `master` e conferida numa partida real completa contra bots: linha do tempo, habilidades, inventário e fim de partida conferem com o log. Registrados o `matchid` real no `POST_GAME`, a decisão de manter o último herói e os dados da última partida na aba Herói, a tela de partida no Histórico e os pontos em aberto de nomes de itens e da linha do tempo.
- 2026-10-24 — Pendências da Fase 2 implementadas: relógio e ID da partida no estado, catálogo de itens embutido, TP fora da linha do tempo, itens neutros, montagem de itens pela receita, resumo e última partida na aba Herói com estatísticas e itens, e fixture real de fim de partida. Corrigidos a detecção de partida após `WAIT_FOR_PLAYERS_TO_LOAD` e o ID que chega depois do primeiro `POST_GAME`. Registradas as decisões de configuração no `config.toml`, botão manual reiniciando a linha do tempo e TP fora da linha do tempo, e o desempenho da interface com eventos muito rápidos.
- 2026-10-24 — Fechamento da Fase 2:
  - interface com coalescência de estados e atualização incremental das abas Herói e Diagnóstico;
  - diagnósticos concorrentes numerados numa só sequência, publicando só a rodada mais recente;
  - teste intermitente de instalação corrigido.

  Fase 2 concluída, com a conferência visual e em partida real registrada como pendente. A Fase 3 aguarda confirmação do mantenedor para ser marcada como atual.
- 2026-10-24 — Fase 3 marcada como atual, a pedido do mantenedor. Ela começa pela conferência no jogo pendente da Fase 2. O relógio da partida em cada evento da linha do tempo entra na Fase 4. O tipo de partida no GSI fica em aberto, para conferir em partidas que não sejam contra bots.
- 2026-10-25 — Partidas de teste contra bots e turbo: o GSI não informa o modo de jogo. Decidida e incluída na Fase 3 a escolha manual do modo ao encontrar a partida, como pré-requisito das recomendações.
- 2026-10-25 — Plano completo da Fase 3 revisado: builds profissionais e públicas por modo, algoritmo da build, orçamento de chamadas como política do app, coleta pública em segundo plano, migração do banco para a versão 2 e proteção contra respostas antigas. A STRATZ saiu da fase para a milestone "Fontes de dados extras".
- 2026-10-25 — Fase 3 implementada: dados embutidos com custo, ID dos itens, IDs de habilidades e patches; agregação de builds; banco na versão 2; orçamento de chamadas; cliente HTTP com fallback; patch atual; builds profissionais; coleta pública em segundo plano; recomendações no núcleo com proteção contra respostas antigas; abas Itens, Habilidades e Dados profissionais. Falta a conferência numa partida real (#27) para concluir a fase.
- 2026-10-25 — A pedido do mantenedor, a agregação de builds (item grande, amostra mínima, taxa de vitória e ordem de habilidades mais comum) foi substituída pela build real das vitórias mais recentes: 3 profissionais e 5 públicas do modo, com inventário final, mochila, neutro e ordem de compra completa, e a ordem real de habilidades (#56). O banco foi para a versão 3.
- 2026-10-26 — Partida real da #27 (Drow Ranger, Escolha Livre) conferida pela acessibilidade e por captura de tela; achados viraram as issues #57 e #58. Decidida a página única da build na aba Herói, com ícones dos itens e progresso da partida (#59).
- 2026-10-27 — Fase 3 reorganizada em torno da página única da aba Herói (#59, #62 e #63): sem as abas Itens, Habilidades e Dados profissionais, sem linha do tempo, popularidade, resumo, qualidade dos dados, estatísticas, níveis, card do modo e filtro por fonte (decisões do mantenedor, uma por uma, durante a revisão visual). O modo de jogo não pode mais ser alterado depois que a partida é encontrada. Banco na versão 5. Plano e testes atualizados para o estado do código; a Fase 3 continua como atual, aguardando as conferências em partida real.
