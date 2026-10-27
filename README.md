# Dota Beginner Companion

Aplicativo desktop para Linux que acompanha jogadores iniciantes de Dota 2. Ele roda em uma janela própria e recebe o estado do jogo pelo **Game State Integration (GSI)**, o mecanismo oficial do Dota, por um servidor local em `127.0.0.1`.

Feito para Arch Linux com KDE Plasma (Wayland), em Rust com GTK4 + Relm4.

## Funcionalidades

A aba **Herói** começa pela página da build, com dados do [OpenDota](https://www.opendota.com/) do patch atual para o herói e o modo da partida:

- à esquerda, as 3 vitórias mais recentes do herói em partidas profissionais de liga e as 5 mais recentes em partidas públicas do modo, só com título, data, duração e, à direita, o K/D/A do jogador naquela vitória; escolha uma para acompanhá-la (a escolha fica só no aplicativo e não muda nada no jogo).
- à direita, a build real da vitória escolhida, como o jogador fez: itens iniciais, a ordem dos itens completos com o que você já comprou (✓) e o próximo (→), itens finais, mochila e item neutro com o aprimoramento; o aplicativo não monta builds nem esconde itens;
- para as vitórias profissionais, o card "Você × build" compara as suas médias por minuto (last hits, denies, ouro e experiência) com as do jogador da build, com a diferença em porcentagem e "à frente" ou "atrás" em texto;
- a grade da ordem de habilidades da vitória, ponto a ponto, com os pontos que você já gastou;
- em cima, o herói com o atributo e as funções (dados fixos do herói), o botão "Partida encontrada" (marca a partida quando o aplicativo não a deduziu) e o patch e a fonte e os avisos (inclusive de dados antigos); Contra bots usa os dados de Escolha Livre; as partidas não são separadas por posição (1 a 5); ao escolher uma vitória profissional em Turbo, um aviso lembra que ela é de uma partida normal (as vitórias públicas de Turbo são de Turbo, sem aviso);

As imagens dos itens, o retrato do herói e os ícones das habilidades são baixados do CDN da Steam na primeira vez e guardadas em `~/.cache/dota-companion/item-icons/`; sem rede, tudo aparece pelo nome. As imagens são da Valve e não fazem parte do repositório.

Na mesma página, em cards, o aplicativo acompanha a partida pelo GSI:

- quando a partida é encontrada, pergunta numa janela do próprio aplicativo o modo de jogo (Escolha Livre, Turbo ou Contra bots), que o GSI não informa; a escolha vem marcada com o último modo confirmado e vale até a partida acabar (o modo não muda no meio do jogo, então a página não tem botão para alterá-lo);
- mostra o herói escolhido com atributo, tipo de ataque e funções, a partir de dados embutidos do [dotaconstants](https://github.com/odota/dotaconstants) (sem rede), que também fornecem os nomes e as receitas dos itens;
- usa as suas estatísticas da partida (last hits, denies, GPM e XPM) só para a comparação com a build, e os itens carregados para marcar o progresso da ordem de compra;
- usa os pontos de habilidade gastos, informados pelo GSI, para marcar o progresso na grade da ordem de habilidades da build;
- monta a linha do tempo da partida (ainda sem tela: ela vai para a aba Histórico): partida encontrada, herói escolhido, início e fim da partida, níveis de habilidade, itens que entram ou saem, itens neutros, itens montados a partir dos componentes e mortes (o TP fica de fora);
- depois que a partida termina, mostra o resumo dela (resultado, duração, estatísticas, itens finais e o ID da partida, quando o Dota o envia) até a próxima partida ser encontrada;
- indica a origem de cada informação: dado embutido, confirmado pelo GSI, inferido pelo aplicativo ou marcado por você;
- tem o botão **Partida encontrada**, para marcar esse momento manualmente, já que o GSI não o informa.

As vitórias profissionais recentes com o herói (liga, data e duração) aparecem na lista de builds da aba **Herói**; não há aba separada para elas. As abas ficam no topo da janela e, em janela estreita, numa barra embaixo.

As partidas públicas são coletadas em segundo plano enquanto o aplicativo está aberto. O aplicativo limita as próprias chamadas ao OpenDota (por padrão, 1.000 por dia, com até 700 para a coleta) e guarda as respostas no banco local.

A tela **Diagnóstico** é uma tela de saúde do aplicativo. No alto, o estado geral (Tudo funcionando, Atenção necessária, Não pronto ou Núcleo indisponível) e, abaixo, um card para cada coisa que precisa funcionar:

- **Steam:** "Estado desconhecido" por enquanto, porque o aplicativo ainda não verifica se a Steam está aberta (issue #65);
- **Dota 2:** em execução, não detectado (pela lista de processos do sistema) ou instalação não encontrada (o app procura nas bibliotecas Steam: padrão, Flatpak, `libraryfolders.vdf` e extras);
- **Servidor GSI:** funcionando, iniciando ou com erro;
- **Configuração GSI:** instalada e válida, ausente, apontando para outro endereço ou porta, com token divergente ou inválida;
- **Recebimento de dados:** recebendo dados (último evento há até 90 s), aguardando o Dota, sem dados recentes ou conexão instável ou interrompida.

Três ações, cada uma com uma explicação do que faz: **Instalar configuração GSI** (com um clique; se já existir um arquivo diferente, pede confirmação e faz backup antes de substituir), **Testar conexão** (envia um evento de teste ao próprio servidor) e **Rodar diagnóstico novamente**. O resultado das ações aparece logo abaixo dos cards.

Os números técnicos (eventos recebidos e gravados, requisições recusadas, a gravação em arquivo, o caminho da configuração do aplicativo e o último JSON recebido, sem o token) ficam num bloco fechado, **Detalhes técnicos**.

A barra de título mostra o estado da conexão GSI.

## Requisitos

- Arch Linux (ou outra distribuição com GTK 4 e libadwaita)
- `gtk4`, `libadwaita`, `pkgconf`
- Rust 1.99.1, via [asdf](https://asdf-vm.com/) (`.tool-versions`) ou rustup (`rust-toolchain.toml`)

```sh
sudo pacman -S gtk4 libadwaita pkgconf
asdf plugin add rust
asdf install
```

## Executar

```sh
cargo run              # abre o aplicativo
cargo test             # testes automatizados
scripts/send_test_event.sh # envia um evento sintético ao app aberto
scripts/replay_gsi_log.sh  # reenvia ao app aberto o log gravado em logs/gsi.jsonl
scripts/check.sh       # todas as verificações (ver CONTRIBUTING.md)
RUST_LOG=debug cargo run   # com logs detalhados
```

## Configurar o GSI

1. Abra o aplicativo, vá em **Diagnóstico** e clique em **Instalar configuração GSI**.
2. Na Steam: Dota 2 → Propriedades → Geral → Opções de inicialização, adicione `-gamestateintegration`.
3. Reinicie o Dota 2 e entre em uma partida ou lobby (contra bots também funciona).
4. O Diagnóstico passa a mostrar "Recebendo dados".

## Teste manual sem o Dota

Com o aplicativo aberto, execute `scripts/send_test_event.sh` em outro terminal. O script compila e executa o exemplo Rust `send_test_event`, envia um único evento sintético e informa sucesso ou diagnóstico de falha. O evento fica separado dos eventos do jogo no Diagnóstico.

A configuração segue a mesma prioridade do aplicativo: ambiente do processo, `.env` no diretório de execução e `config.toml`. Execute a partir do mesmo diretório usado para abrir o app. Se o `config.toml` ainda não existir, ele é criado como na inicialização normal. O script não imprime o token nem interpreta o `.env` como código shell. Falhas retornam código de saída 1.

Para reproduzir uma partida inteira sem o jogo, use um log gravado com `DOTA_COMPANION_GSI_LOG` e execute, com o aplicativo aberto:

```sh
scripts/replay_gsi_log.sh                         # logs/gsi.jsonl, 100 ms entre eventos
scripts/replay_gsi_log.sh outro.jsonl --interval-ms 20
```

Os eventos são reenviados como eventos do Dota, com o token da configuração, e passam pelo mesmo caminho do jogo: aparecem no Diagnóstico e na aba Herói. O script mostra só a contagem de respostas, nunca o conteúdo dos eventos, que tem SteamID e nome do jogador.

## Configuração

Arquivo `~/.config/dota-companion/config.toml`. Ele é criado na primeira execução com permissão `0600`.

| Chave             | Padrão                        | Uso                                        |
| ----------------- | ----------------------------- | ------------------------------------------ |
| `gsi_port`        | `53000`                       | Porta do servidor GSI local                |
| `gsi_token`       | gerado automaticamente        | Token que o Dota envia em cada evento      |
| `extra_libraries` | `["/mnt/jogos/SteamLibrary"]` | Bibliotecas Steam fora dos caminhos padrão |

As variáveis de ambiente abaixo têm prioridade sobre o arquivo. Elas também podem ficar em um `.env` no diretório de execução (veja o `.env.example`). Uma variável definida no ambiente vale mais que a do `.env`.

- `DOTA_COMPANION_GSI_PORT`: porta do servidor.
- `DOTA_COMPANION_EXTRA_LIBRARIES`: bibliotecas extras, separadas por `:`.
- `DOTA_COMPANION_GSI_LOG`: arquivo onde cada evento recebido do Dota é gravado em JSON Lines, para desenvolvimento. Acompanhe com `tail -f logs/gsi.jsonl | jq`. O arquivo contém SteamID e nome do jogador; a pasta `logs/` não é versionada. Vazio não ativa a gravação.
- `RUST_LOG`: nível de log (`error`, `warn`, `info`, `debug`, `trace`). Sem valor, o log mostra só erros; um valor inválido também mostra só erros, começando por um que aponta o `RUST_LOG` inválido.

O `.env` é opcional e só é procurado no diretório de execução. Se ele existir e não puder ser lido ou tiver uma linha inválida, o aplicativo não inicia e mostra no terminal a linha e a coluna do problema, sem o conteúdo da linha.

Se mudar a porta ou o token, reinstale o arquivo GSI pelo Diagnóstico.

## Documentação

Arquitetura e padrões do projeto estão em [`docs/`](docs/README.md). Para contribuir, veja [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Licença

[GPL-3.0-or-later](LICENSE).
