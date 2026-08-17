use rand::RngExt;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ObfuscatedExpr(pub String);

fn random_text() -> String {
    let mut rnd = rand::rng();
    let mut buffer = String::with_capacity(10);
    for _ in 0..10 {
        buffer.push(rnd.random_range(65..90) as u8 as char);
    }
    buffer
}

impl ObfuscatedExpr {
    pub fn new() -> Self {
        ObfuscatedExpr(random_text())
    }

    pub fn generate_marker(&self) -> String {
        let mut str = self.0.clone();
        str.insert_str(0, "<!-- marker:");
        str += " -->";
        str
    }

    // pub fn generate_marker_pairs(&self) -> (String, String) {
    //     let mut str_start = self.0.clone();
    //     let mut str_end = self.0.clone();
    //     str_start.insert_str(0, "<!-- marker:start__");
    //     str_start += " -->";
    //     str_end.insert_str(0, "<!-- marker:end__");
    //     str_end += " -->";
    //     (str_start, str_end)
    // }
}
