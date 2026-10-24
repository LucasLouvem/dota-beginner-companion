# Padrão de documentação

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Regras para escrever e manter o `README.md` e os documentos em `docs/`.

## Princípio

A documentação é um **espelho do projeto como ele está agora**. Ela responde três perguntas:

1. O que é?
2. Para que serve?
3. Como usar?

## O que entra

- Funcionalidades e componentes que existem no código atual.
- Comportamentos e contratos estáveis: fluxos, regras, formatos, configurações, comandos.
- Informação que ajuda a usar ou modificar o projeto corretamente.

## O que fica de fora

- Funcionalidades não implementadas, planos e roteiros.
- Histórico, decisões passadas, alternativas descartadas e conversas.
- Listas do que o projeto não faz.
- Cópia de código: trechos do projeto, assinaturas completas, listas de funções ou de arquivos que mudam com frequência. Descreva o comportamento; o código é a fonte dos detalhes.

Exemplos genéricos são permitidos quando ilustram um padrão (ex.: formato de commit). Eles não podem ser trechos copiados do projeto.

### Exceção: decisões aprovadas

O [`ARCHITECTURE.md`](ARCHITECTURE.md) tem uma seção "Decisões aprovadas e não implementadas". É o único lugar da documentação onde pode aparecer algo aprovado que ainda não existe no código. Cada item sai dessa seção quando é implementado.

## Cabeçalho

Todo documento em `docs/` começa com:

```markdown
# Título do documento

**Criado em:** AAAA-MM-DD · **Atualizado em:** AAAA-MM-DD

Uma frase dizendo para que serve este documento.
```

- Datas no formato ISO (`AAAA-MM-DD`).
- `Criado em` nunca muda.
- `Atualizado em` muda sempre que o conteúdo do documento muda.

O `README.md` da raiz não tem cabeçalho de datas. Ele apresenta o projeto e aponta para `docs/`.

## Quando atualizar

Atualize no mesmo commit da mudança de código quando ela alterar:

- comportamento visível ao usuário (README);
- camadas, ports, fluxo de eventos ou concorrência (ARCHITECTURE);
- erros tratados ou suas mensagens e ações (ERRORS);
- regras ou comandos de teste (TESTING);
- configuração, variáveis de ambiente ou comandos de execução (README).

Mudanças internas que não alteram nenhum desses pontos não exigem atualização.

## Verificação automática

Todo arquivo Markdown do repositório passa por duas ferramentas, e as duas precisam passar antes de cada commit:

- **dprint** formata Markdown e TOML (configuração em `dprint.json`). Corrigir com `dprint fmt`.
- **rumdl** verifica a estrutura do Markdown: títulos, listas, blocos de código com linguagem e links (configuração em `.rumdl.toml`). Corrigir com `rumdl fmt .`.

Os comandos estão em [TESTING.md](TESTING.md#comandos).

## Estilo

- Português (PT-BR), frases curtas e diretas.
- Tabelas e listas em vez de parágrafos longos.
- Links relativos entre documentos (`[Erros](ERRORS.md)`).
- Um assunto por documento. Novo assunto ganha novo arquivo e entra no índice [`docs/README.md`](README.md).
