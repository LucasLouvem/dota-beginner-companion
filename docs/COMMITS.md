# Padrão de commits

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Formato obrigatório das mensagens de commit.

## Formato

O projeto segue [Conventional Commits](https://www.conventionalcommits.org/). As mensagens são **em inglês**.

```text
type(scope): subject

body (opcional)

footer (opcional)
```

- **subject**: no imperativo ("add", não "added"), em minúsculas, sem ponto final, com até 72 caracteres.
- **body**: explica o motivo da mudança e o seu impacto, não o passo a passo. Linhas de até 72 caracteres.
- **footer**: `BREAKING CHANGE: <descrição>` para mudanças incompatíveis, e referências como `Refs: #12`.
- Cada commit trata de um único assunto. Não misture refatoração com funcionalidade nova.

## Tipos

| Tipo       | Uso                                            |
| ---------- | ---------------------------------------------- |
| `feat`     | Funcionalidade nova para o usuário             |
| `fix`      | Correção de bug                                |
| `docs`     | Somente documentação                           |
| `test`     | Testes novos ou corrigidos                     |
| `refactor` | Mudança interna sem alterar comportamento      |
| `perf`     | Melhoria de desempenho                         |
| `style`    | Formatação, sem mudança de lógica              |
| `build`    | Cargo, dependências, empacotamento             |
| `ci`       | Pipeline de integração contínua                |
| `chore`    | Manutenção que não se encaixa nos outros tipos |
| `revert`   | Reverte um commit anterior                     |

## Escopos

Use o nome da camada afetada: `domain`, `application`, `gsi`, `infrastructure`, `presentation`. Há também os escopos `docs` e `deps`. Se a mudança atinge várias camadas de forma equivalente, omita o escopo.

## Exemplos

Certo:

```text
feat(gsi): reject requests larger than 1 MB
fix(infrastructure): skip unreadable libraryfolders.vdf entries
docs: describe gsi setup in readme
test(application): cover install conflict without confirmation
build(deps): bump axum to 0.8.9
```

Com corpo e quebra de compatibilidade:

```text
refactor(infrastructure)!: rename gsi_port config key

The key now matches the environment variable name.

BREAKING CHANGE: existing config.toml files must rename gsi_port.
```

Errado:

```text
Fixed stuff                     # sem tipo, vago, passado
feat: Added new screen.         # passado, maiúscula, ponto final
update(gsi): changes            # tipo inexistente, assunto vago
feat(gsi): add server and fix ui and update docs   # vários assuntos
```
