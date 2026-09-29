import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    
    // Configura o tamanho mínimo e o tamanho inicial (1440x900) para estilo NLE
    let initialRect = NSRect(x: 0, y: 0, width: 1440, height: 900)
    self.contentViewController = flutterViewController
    self.setFrame(initialRect, display: true)
    
    // Centraliza a janela no monitor
    self.center()
    
    // Define um tamanho mínimo para que o usuário não esmague a Timeline
    self.minSize = NSSize(width: 1024, height: 768)

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
  }
}
