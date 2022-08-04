use std::process::Command;

use crate::{App, pod, Pod, StatefulList};

pub fn populate_pods(app: &mut App) {
    let output = Command::new("oc")
        .arg("get")
        .arg("pods")
        .arg("-o")
        .arg("json")
        .output()
        .expect("ls command failed to start");
    match output.status.success() {
        true => {
            let result: Result<pod::pods::Pods, _> = serde_json::from_str(String::from_utf8_lossy(&output.stdout).to_string().as_str());

            let pods = match result {
                Ok(l) => { l }
                Err(err) => {
                    println!("{}", err.to_string());
                    return;
                }
            };
            app.pods = StatefulList::with_items(pods.items.iter()
                .map(|p| { Pod { name: p.metadata.name.clone() } }).collect());
        }
        false => {
            println!("{}", String::from_utf8_lossy(&output.stderr));
        }
    }
}
