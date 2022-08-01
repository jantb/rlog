use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize)]
pub(crate) struct Pods {
    pub(crate) items: Vec<Item>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct Item {
    pub(crate) metadata: Metadata,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct Metadata {
    pub(crate) name: String,
}
