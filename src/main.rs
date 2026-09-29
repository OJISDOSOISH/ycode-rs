use ycode::core::run::Run;
use ycode::llm::Llm;
use ycode::schema::session_message::ModelRef;
use ycode::tool::{ToolBox, Workspace};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Petit modele heberge gratuit, choisi pour un essai rapide.
    let model = ModelRef::new("nvidia", "nemotron-3.5-lightning-30b-a3b");
    let llm = Llm::new(model)?;

    let workspace = Workspace::new(std::env::current_dir()?)?;
    let tools = ToolBox::new(workspace);

    let prompt = std::env::args().nth(1).unwrap_or_else(|| {
        "Cree un fichier hello.rs contenant une fonction qui renvoie \"bonjour\".".to_string()
    });

    let mut run = Run::new(&llm, &tools);
    let answer = run.execute(&prompt, 200_000, 8_000).await?;

    println!("\n=== reponse de l'agent ===\n{answer}");
    println!("\n=== {} tours, {} messages en historique ===", run.turns(), run.entries.len());
    Ok(())
}

