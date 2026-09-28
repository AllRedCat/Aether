# Original User Request

## Initial Request — 2026-09-28T03:08:43Z

# Teamwork Project Prompt — Draft

> Status: Launched
> Goal: Craft prompt → get user approval → delegate to teamwork_preview
> Requested team: Equipe completa (Múltiplos agentes para arquitetura, construção e revisão paralela)

Implementar um Full-stack Slice do Aether: Desenvolver o fluxo completo da funcionalidade de "Adicionar Clipe" na timeline, conectando a interface em Flutter (Riverpod) ao modelo não-destrutivo (DAG) do motor em Rust utilizando a ponte FFI.

Working directory: `/Users/gabrielgenaro/Developer/Pessoal/Aether`
Integrity mode: development

## Requirements

### R1. Lógica do Motor Nativo (Rust Core & Bridge)
Implementar a funcionalidade em `aether_core` para instanciar e adicionar um novo `Clip` a uma `Track` específica dentro do objeto `Timeline`. Deve calcular corretamente as posições temporais (PTS) da linha do tempo. Expor essa funcionalidade via `flutter_rust_bridge` no pacote `aether_bridge`.

### R2. Interface e Integração (Flutter App)
Construir um gerenciador de estado (Provider no Riverpod) em `aether_app` que consuma o estado da Timeline do Rust. Desenvolver um botão ou interface básica na UI que acione o método FFI gerado para adicionar um clipe e atualize a view (TimelineView) mostrando a contagem de clipes atual.

## Acceptance Criteria

### Rust Core Tests (Automated Verification)
- [ ] O comando `cargo test -p aether_core` deve passar sem falhas, contendo ao menos um teste unitário que valide se um `Clip` foi adicionado com sucesso a uma `Track` e se o tamanho da Timeline (`duration_pts`) foi recalculado corretamente.
- [ ] O pacote compila com sucesso (`cargo check -p aether_bridge`).

### Flutter Bridge & UI Tests (Automated Verification)
- [ ] O código FFI deve ser gerado sem erros usando o comando `make bridge` (flutter_rust_bridge_codegen).
- [ ] O aplicativo Flutter não deve apresentar erros estáticos (`flutter analyze` limpo).
- [ ] (Verificação por Agente Juiz) O código Dart da tela de Timeline deve comprovadamente ler a lista de faixas/clipes recebida do Rust via FFI e exibir a quantidade correta em tela.
