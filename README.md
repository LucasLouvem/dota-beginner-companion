# Dota Beginner Companion

Aplicativo desktop para Linux que acompanha jogadores iniciantes de Dota 2. Ele roda em uma janela própria e recebe o estado do jogo pelo **Game State Integration (GSI)**, o mecanismo oficial do Dota, por um servidor local em `127.0.0.1`.

Feito para Arch Linux com KDE Plasma (Wayland), em Rust com GTK4 + Relm4.

## Funcionalidades

A aba **Herói** acompanha a partida pelo GSI:

- mostra o herói escolhido com atributo, tipo de ataque e funções, a partir de dados embutidos do [dotaconstants](https://github.com/odota/dotaconstants) (sem rede);
- lista as habilidades com o nível atual informado pelo GSI;
- monta a linha do tempo da partida: partida encontrada, herói escolhido, início e fim da partida, níveis de habilidade, itens que entram ou saem do inventário e mortes;
- indica a origem de cada informação: dado embutido, confirmado pelo GSI, inferido pelo aplicativo ou marcado por você;
- tem o botão **Partida encontrada**, para marcar esse momento manualmente, já que o GSI não o informa.

A tela **Diagnóstico** mostra se tudo está pronto para o GSI funcionar:

- localiza as bibliotecas Steam (padrão, Flatpak, `libraryfolders.vdf` e bibliotecas extras) e a instalação do Dota 2;
- informa se o Dota 2 está aberto, pela lista de processos do sistema;
- instala o arquivo `gamestate_integration_dota_companion.cfg` com um clique. Se já existir um arquivo diferente, pede confirmação e faz backup antes de substituir;
- confere se o arquivo instalado usa a porta e o token atuais do aplicativo;
- mostra o servidor GSI local, os eventos recebidos do Dota, as requisições recusadas e o horário do último evento;
- tem o botão **Testar conexão**, que envia um evento de teste ao próprio servidor;
- exibe o último JSON recebido (sem o token).

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
4. O Diagnóstico passa a mostrar os eventos recebidos.

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
