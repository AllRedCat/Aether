use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::timeline::Timeline;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub project_path: String, // Caminho absoluto para a PASTA do projeto
    pub file_path: String,    // Caminho absoluto para o ARQUIVO .aether
    pub timeline: Timeline,
}

impl Project {
    /// Cria um novo projeto não-destrutivo.
    /// `base_dir` é a pasta padrão de documentos (enviada pelo Flutter de acordo com o OS: Mac, Win, Android, etc)
    pub fn create_new(name: &str, base_dir: &str) -> Result<Self, String> {
        let project_id = Uuid::new_v4().to_string();
        
        // Remove espaços e caracteres especiais para nome de pasta seguro
        let safe_name = name.replace(" ", "_").to_lowercase();
        
        // Ex: /Users/gabriel/Documents/Aether/meu_video/
        let project_dir = Path::new(base_dir).join(&safe_name);
        if !project_dir.exists() {
            fs::create_dir_all(&project_dir).map_err(|e| format!("Erro ao criar pasta: {}", e))?;
        }
        
        // Ex: /Users/gabriel/Documents/Aether/meu_video/meu_video.aether
        let file_path = project_dir.join(format!("{}.aether", safe_name));
        
        let project = Project {
            id: project_id,
            name: name.to_string(),
            project_path: project_dir.to_string_lossy().to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            timeline: Timeline::new(crate::timeline::Rational { num: 60, den: 1 }),
        };
        
        // Já faz o primeiro salvamento em disco
        project.save()?;
        
        Ok(project)
    }

    /// Salva o estado atual (Timeline, Clipes, Edições) no disco usando JSON
    pub fn save(&self) -> Result<(), String> {
        // Pretty print para ficar legível caso a gente queira ler o .aether num bloco de notas
        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Falha ao converter projeto para JSON: {}", e))?;
            
        fs::write(&self.file_path, json_data)
            .map_err(|e| format!("Falha ao escrever arquivo no disco: {}", e))?;
            
        Ok(())
    }

    /// Carrega um projeto existente do disco a partir do caminho do arquivo .aether
    pub fn load(file_path: &str) -> Result<Self, String> {
        let json_data = fs::read_to_string(file_path)
            .map_err(|e| format!("Falha ao ler o arquivo .aether: {}", e))?;
            
        let project: Project = serde_json::from_str(&json_data)
            .map_err(|e| format!("Falha ao decodificar projeto corrompido: {}", e))?;
            
        Ok(project)
    }
}
