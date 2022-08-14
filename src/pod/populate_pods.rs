use std::process::{Command, Output};

use crate::{App, pod, Pod, StatefulList};

pub fn populate_pods(app: &mut App) {
    let output = Command::new("oc")
        .arg("get")
        .arg("pods")
        .arg("-o")
        .arg("json")
        .output()
        .expect("'oc get pods -o json' failed to start");
    populate(app, &output);
}

pub fn populate_topics(app: &mut App) {
    let output = Command::new("java")
        .arg("-jar")
        .arg("kafka.jar")
        .arg("topics")
        .arg("list")
        .output()
        .expect("'java -jar kafka-jar list' failed to start");
    populate(app, &output);
}

fn populate(app: &mut App, output: &Output) {
    app.pods = StatefulList::with_items(vec![]);
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
            app.pods = StatefulList::with_items(pods.items.iter().filter(|pod| pod.status.phase == "Running")
                .map(|p| { Pod { name: p.metadata.name.clone() } }).collect());
        }
        false => {
            println!("{}", String::from_utf8_lossy(&output.stderr));
        }
    }
}
