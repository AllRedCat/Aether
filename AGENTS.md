# AGENTS.md — Diretrizes de Operação do Agente

Este arquivo define regras estritas de execução, fluxo de trabalho e rastreabilidade para o agente neste repositório. Todas as tarefas devem seguir rigorosamente os princípios abaixo.

---

## 1. Planejamento e Decomposição (Micro-etapas)
- **Não execute tudo de uma vez:** Antes de fazer qualquer alteração em arquivos ou no código, quebre a demanda em micro-etapas lógicas e bem delimitadas.
- **Apresente o Checklist:** Inicie a interação exibindo o plano de ação particionado no formato de checklist (`[ ] Etapa 1`, `[ ] Etapa 2`, etc.).
- **Execução Sequencial:** Trabalhe em estritamente **uma micro-etapa por vez**. Nunca avance para o passo seguinte sem concluir, validar e commitar o passo atual.
- **Detalhamento:** Forneça contexto claro e detalhado sobre o que exatamente foi modificado ou adicionado em cada etapa concluída.

---

## 2. Rastreabilidade e Commits Atômicos
- **Commit Imediato:** Assim que uma micro-etapa for finalizada e validada, realize imediatamente o commit dessas alterações antes de prosseguir.
- **Atomicidade:** Cada commit deve conter apenas as modificações estritamente necessárias daquela etapa. Evite misturar múltiplos arquivos ou contextos distintos em um único commit.
- **Padrão de Mensagens:** Utilize mensagens no padrão [Conventional Commits](https://www.conventionalcommits.org/):
  - Formato: `<tipo>(<escopo>): <descrição curta e imperativa>`
  - Exemplos:
    - `feat(auth): adiciona validação de token jwt no middleware`
    - `fix(router): corrige rotação de parâmetros na query`
    - `test(api): adiciona testes unitários para endpoint de login`
    - `refactor(db): isola pool de conexões em módulo singleton`

---

## 3. Validação e Tratamento de Erros
- **Validação Local:** Antes de commitar cada micro-etapa, execute testes locais, linters ou checagens de build pertinentes às alterações feitas.
- **Tratamento de Bloqueios:** Se uma etapa falhar ou gerar erros inesperados:
  1. Pare imediatamente a execução;
  2. Não modifique outros arquivos para tentar mascarar o problema;
  3. Relate o erro, a causa raiz identificada e a proposta de correção para resolução antes de prosseguir.

---

## 4. Postura e Comunicação
- **Objetividade Técnica:** Mantenha explicações técnicas concisas, focando no *porquê* e no *como* das alterações realizadas.
- **Status da Tarefa:** A cada entrega de etapa, informe claramente:
  - O que foi feito;
  - O hash ou mensagem do commit gerado;
  - O próximo item do checklist a ser iniciado.
