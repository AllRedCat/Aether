# 🌌 Aether

O **Aether** é um editor de vídeo não-linear (NLE) multiplataforma de alta performance. Ele foi desenhado para rodar fluidamente em tablets (iPadOS/Android) e desktops (macOS/Windows/Linux).

A filosofia principal do projeto é extrair o melhor de dois mundos: **Flutter** para construir uma interface gráfica incrivelmente responsiva e bonita, e **Rust** para garantir processamento de mídia brutalmente rápido, com segurança de memória e baixo consumo de bateria.

---

## 🏗️ Arquitetura de Alto Nível

O Aether adota uma arquitetura em duas camadas interligadas de forma nativa e sem gargalos:

1. **Camada de Apresentação (Flutter/Dart):** Cuida exclusivamente da interface de usuário, botões, animações, arrastar-e-soltar da linha do tempo e do gerenciamento de estado (Riverpod).
2. **Motor Core (Rust):** O "cérebro" do editor. Gerencia toda a lógica pesada: cálculo de *timestamps* (PTS), grafos não-destrutivos de renderização, decodificação de mídia e acesso ao hardware gráfico.

Elas conversam de forma síncrona e assíncrona através de uma ponte FFI (Foreign Function Interface) gerada magicamente pela ferramenta `flutter_rust_bridge`.

---

## 📂 Estrutura do Repositório (Monorepo)

O projeto está organizado como um *Monorepo*, mantendo o motor (Rust) e o aplicativo (Flutter) no mesmo lugar para versionamento sincronizado.

```text
Aether/
├── apps/
│   └── aether_app/           # O aplicativo frontend em Flutter
│       ├── lib/main.dart     # Ponto de entrada da UI
│       ├── lib/src/features/ # Telas e componentes isolados (Ex: timeline)
│       └── lib/src/bridge/   # Código Dart auto-gerado que conversa com o Rust
│
├── crates/                   # O motor backend em Rust (dividido em micropaquetes)
│   ├── aether_core/          # Lógica pura: Timeline, Tracks, Clips, PTS
│   ├── aether_bridge/        # A "Fronteira": Funções expostas para o Flutter
│   ├── aether_render/        # Renderização gráfica (GPU, wgpu, Metal)
│   └── aether_media/         # Leitura, decoding e manipulação de arquivos de vídeo
│
├── Makefile                  # Comandos úteis de automação
└── Cargo.toml                # Gerenciador de dependências do workspace Rust
```

---

## ⚙️ Como o Código Funciona na Prática?

Para entender a dinâmica, vamos acompanhar a jornada de uma ação simples: **O usuário clica no botão "Adicionar Clipe" na Timeline.**

### 1. A Interface (Dart / Flutter)
Na pasta `apps/aether_app/lib/src/features/timeline`, temos nossa `TimelineView`. 
Quando o botão é clicado, nós não fazemos cálculos de vídeo no Dart. Apenas dizemos ao nosso gerenciador de estado (Riverpod):
> *"Ei Notifier, o usuário quer adicionar um clipe!"*

O `TimelineNotifier` então chama uma função da nossa ponte (API):
```dart
await RustLibApi.instance.addClipToTimeline(clipName: "video_praia.mp4");
```

### 2. A Ponte Invisível (FFI)
A biblioteca `flutter_rust_bridge` serializa esse pedido rapidamente (sem usar JSON, direto na memória) e o envia para as *threads* de background rodando o Rust. O gerenciamento inteligente da ponte permite que o Dart não bloqueie a UI enquanto o Rust trabalha.

### 3. A Fronteira Rust (`aether_bridge`)
O pedido chega no arquivo `crates/aether_bridge/src/api.rs`. O Rust recebe os parâmetros e aciona a engine principal:
```rust
pub fn add_clip_to_timeline(clip_name: String) -> Timeline {
    let mut timeline = get_current_timeline();
    let clip = Clip::new(clip_name);
    timeline.add_clip(clip);
    timeline // Retorna o novo estado
}
```

### 4. O Cérebro (`aether_core`)
Lá no `crates/aether_core/src/timeline.rs`, as estruturas de dados matemáticas de verdade entram em ação. O Rust recalcula o tempo (Presentation Time Stamp - PTS) de forma exata, organiza a estrutura de árvore (DAG) não-destrutiva e garante que nenhum vazamento de memória ocorra, tudo com a performance que um NLE profissional exige.

### 5. O Retorno à Superfície
A `Timeline` atualizada pelo Rust volta pela ponte FFI para o Flutter. O `TimelineNotifier` do Riverpod percebe a mudança de estado, e como num passe de mágica, a tela do usuário é redesenhada com o novo clipe visível na trilha, rodando a fluidos 60 ou 120 FPS.

---

## 🚀 Como Desenvolver e Rodar

### Pré-requisitos
- **Flutter SDK** (Versão 3.x+)
- **Rust Toolchain** (`rustup default stable`)
- **Xcode** (para desenvolvimento no macOS/iOS)
- **Cargokit / FFI Integrado**: A compilação é automática durante o `flutter build`.

### Passos principais
1. **Gere a Ponte FFI** (Sempre que alterar o código em `api.rs` no Rust):
   Na raiz do projeto, rode:
   ```bash
   make bridge
   ```
   *Esse comando garante que as classes Dart tenham exatamente o mesmo formato das classes Rust que você modificou.*

2. **Rode o Aplicativo:**
   Navegue até a pasta do aplicativo e rode:
   ```bash
   cd apps/aether_app
   flutter run -d macos
   ```

Isso é tudo! Com essa fundação, o Aether pode escalar para manipular gigabytes de vídeo e dezenas de trilhas de áudio sem que a UI de toque do iPad ou do Desktop trave.
