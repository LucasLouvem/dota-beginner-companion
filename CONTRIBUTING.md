# Contribuindo

Obrigado por contribuir com o Dota Beginner Companion.

## Ambiente

Dependências do sistema (Arch Linux):

```sh
sudo pacman -S gtk4 libadwaita pkgconf
```

Rust, por um destes caminhos:

- **asdf**: `asdf plugin add rust && asdf install`. A versão vem de `.tool-versions`.
- **rustup**: a versão e os componentes vêm de `rust-toolchain.toml` e são instalados automaticamente no primeiro `cargo`.

Ferramentas de verificação:

- **dprint** e **typos**: `asdf plugin add dprint && asdf plugin add typos && asdf install`. As versões vêm de `.tool-versions`.
- **rumdl**: `cargo install rumdl --locked --version 0.3.0` (com asdf, rode `asdf reshim rust` depois).

Variáveis de ambiente opcionais: copie `.env.example` para `.env`.

## Antes de abrir um PR

Todas as verificações precisam passar ([detalhes](docs/TESTING.md#comandos)):

```sh
scripts/check.sh
```

## Padrões

| Assunto                                   | Documento                                          |
| ----------------------------------------- | -------------------------------------------------- |
| Fase atual do projeto                     | [`plans/`](plans/)                                 |
| Arquitetura e camadas                     | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)     |
| Testes (sucesso e erro obrigatórios)      | [`docs/TESTING.md`](docs/TESTING.md)               |
| Erros                                     | [`docs/ERRORS.md`](docs/ERRORS.md)                 |
| Padrão de código e reaproveitamento       | [`docs/CODE_STANDARDS.md`](docs/CODE_STANDARDS.md) |
| Comentários                               | [`docs/CODE_COMMENTS.md`](docs/CODE_COMMENTS.md)   |
| Documentação                              | [`docs/DOCUMENTATION.md`](docs/DOCUMENTATION.md)   |
| Commits (Conventional Commits, em inglês) | [`docs/COMMITS.md`](docs/COMMITS.md)               |

## Pull requests

- Um assunto por PR, dentro da fase atual do plano.
- Testes e documentação afetada no mesmo PR.
- Descreva o que muda para o usuário e como verificar.

Ao contribuir, você concorda que o seu código seja distribuído sob a licença [GPL-3.0-or-later](LICENSE).
