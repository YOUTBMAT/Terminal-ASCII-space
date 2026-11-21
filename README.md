# Terminal Animated Themes

Uma coleção extensível de wallpapers animados em TrueColor para terminal. O runtime hospeda um shell em PTY e compõe o tema escolhido atrás da saída do shell; células vazias deixam o wallpaper visível, enquanto texto, cores explícitas e o cursor do CLI permanecem em primeiro plano.

## Requisitos

- Rust 1.75 ou mais recente
- Terminal com suporte a cores TrueColor e Unicode

## Instalação e execução

Dentro do diretório do projeto, instale a versão local:

```sh
cargo install --path .
space-terminal
```

Ou compile e execute sem instalar:

```sh
cargo build --release
cargo run --release
```

O programa inicia o shell definido por `$SHELL` (ou `powershell.exe` no Windows) dentro do compositor.

## Temas

| ID | Descrição |
| --- | --- |
| `space` | Espaço profundo com nebulosas, galáxias, estrelas, planetas, sistemas solares, naves e meteoros. |
| `aurora` | Faixas animadas de luz polar em tons de teal, esmeralda e violeta. |
| `ocean` | Fundo marinho em camadas com leito, cardumes, bolhas, tubarões e fauna tropical. |

## Argumentos e atalhos

```sh
space-terminal --theme ocean
space-terminal --theme space --fps 60
space-terminal --list
space-terminal --help
```

- `--theme <id>` inicia com `space` (padrão), `aurora` ou `ocean`.
- `--list` mostra os temas registrados e encerra.
- `--fps <valor>` configura a taxa em foco, de 1 a 120 FPS (padrão: 30). Em abas sem foco, o runtime reduz a animação para economizar recursos.
- `Alt+T` troca para o próximo tema sem reiniciar o shell.
- `Alt+M` abre o seletor de temas; navegue com as setas, confirme com `Enter` ou feche com `Esc`.
- No tema `space`, `F5` alterna o sistema solar, `F9` alterna o warp drive e `F4` ou `Ctrl+Alt+P` troca a paleta.
- `F12` ou `Ctrl+Alt+S` pausa/retoma a animação; `F10` encerra o compositor.

O shell padrão vem de `$SHELL` no Unix ou `COMSPEC` no Windows. As teclas que não são atalhos do compositor são encaminhadas ao PTY.

## Criando um tema

1. **Implemente o tema:** crie `src/themes/<id>.rs` e implemente `engine::Theme`, incluindo `id`, `name`, `description`, `resize`, `update` e `render` usando `engine::Canvas`.
2. **Registre-o:** adicione o módulo, metadata e factory em `src/themes/mod.rs`; o registro habilita `--theme`, `--list` e o seletor dinâmico.
3. **Valide:** execute `cargo fmt --check`, `cargo test` e `cargo clippy --all-targets -- -D warnings`; confira também resize e terminais pequenos.

## Compatibilidade e desempenho

O processo filho roda em um PTY interpretado pelo emulador VT100. Há suporte a alternate screen, rolagem, cores ANSI/256/TrueColor, atributos comuns, caracteres largos, cursor, bracketed paste e protocolos usuais de mouse. O emulador não é um xterm completo; imagens, extensões específicas e alguns relatórios de dispositivo podem não ser suportados.

O renderer reutiliza buffers, agrupa células contíguas de mesmo estilo e redesenha apenas diferenças. A animação trabalha em qualidade máxima quando a janela está em foco e reduz a taxa quando perde foco; o tema espacial também reaproveita o ruído de nebulosa entre frames. Os caminhos normais, panic e sinais tratáveis restauram o estado do terminal; `SIGKILL` e falhas externas não permitem garantir essa restauração.
