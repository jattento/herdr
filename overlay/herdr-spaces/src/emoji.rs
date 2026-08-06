//! Curated emoji choices for space labels.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmojiEntry {
    pub glyph: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
}

impl EmojiEntry {
    pub fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        query.split_whitespace().all(|needle| {
            self.name.contains(needle)
                || self.aliases.iter().any(|alias| alias.contains(needle))
                || self.glyph == needle
        })
    }
}

pub const CATALOG: &[EmojiEntry] = &[
    EmojiEntry {
        glyph: "🔑",
        name: "key",
        aliases: &["access", "credential", "secret"],
    },
    EmojiEntry {
        glyph: "🚀",
        name: "rocket",
        aliases: &["launch", "ship", "deploy"],
    },
    EmojiEntry {
        glyph: "🔧",
        name: "wrench",
        aliases: &["tool", "fix", "maintenance"],
    },
    EmojiEntry {
        glyph: "🔥",
        name: "fire",
        aliases: &["hot", "urgent", "flame"],
    },
    EmojiEntry {
        glyph: "🧠",
        name: "brain",
        aliases: &["ai", "think", "intelligence"],
    },
    EmojiEntry {
        glyph: "🤖",
        name: "robot",
        aliases: &["bot", "agent", "automation"],
    },
    EmojiEntry {
        glyph: "💰",
        name: "money",
        aliases: &["cash", "finance", "revenue"],
    },
    EmojiEntry {
        glyph: "📖",
        name: "book",
        aliases: &["docs", "documentation", "read"],
    },
    EmojiEntry {
        glyph: "🏠",
        name: "house",
        aliases: &["home", "real estate", "property"],
    },
    EmojiEntry {
        glyph: "📈",
        name: "chart",
        aliases: &["analytics", "growth", "metrics"],
    },
    EmojiEntry {
        glyph: "🐛",
        name: "bug",
        aliases: &["issue", "debug", "defect"],
    },
    EmojiEntry {
        glyph: "🔒",
        name: "lock",
        aliases: &["security", "private", "protected"],
    },
    EmojiEntry {
        glyph: "⚙️",
        name: "gear",
        aliases: &["settings", "config", "system"],
    },
    EmojiEntry {
        glyph: "📦",
        name: "package",
        aliases: &["box", "crate", "release"],
    },
    EmojiEntry {
        glyph: "☁️",
        name: "cloud",
        aliases: &["hosting", "infra", "server"],
    },
    EmojiEntry {
        glyph: "🗄️",
        name: "database",
        aliases: &["db", "storage", "data"],
    },
    EmojiEntry {
        glyph: "📱",
        name: "phone",
        aliases: &["mobile", "app", "device"],
    },
    EmojiEntry {
        glyph: "✉️",
        name: "mail",
        aliases: &["email", "message", "inbox"],
    },
    EmojiEntry {
        glyph: "⭐",
        name: "star",
        aliases: &["favorite", "important", "featured"],
    },
    EmojiEntry {
        glyph: "❤️",
        name: "heart",
        aliases: &["love", "favorite", "health"],
    },
    EmojiEntry {
        glyph: "✅",
        name: "check",
        aliases: &["done", "success", "complete"],
    },
    EmojiEntry {
        glyph: "⚠️",
        name: "warning",
        aliases: &["alert", "risk", "caution"],
    },
    EmojiEntry {
        glyph: "💡",
        name: "light bulb",
        aliases: &["idea", "innovation", "insight"],
    },
    EmojiEntry {
        glyph: "🧪",
        name: "test tube",
        aliases: &["test", "experiment", "lab"],
    },
    EmojiEntry {
        glyph: "🔬",
        name: "microscope",
        aliases: &["research", "inspect", "science"],
    },
    EmojiEntry {
        glyph: "🧰",
        name: "toolbox",
        aliases: &["tools", "devtools", "utility"],
    },
    EmojiEntry {
        glyph: "🔨",
        name: "hammer",
        aliases: &["build", "construct", "tool"],
    },
    EmojiEntry {
        glyph: "🪛",
        name: "screwdriver",
        aliases: &["tool", "repair", "hardware"],
    },
    EmojiEntry {
        glyph: "🧱",
        name: "brick",
        aliases: &["build", "foundation", "block"],
    },
    EmojiEntry {
        glyph: "🏗️",
        name: "construction",
        aliases: &["build", "work in progress", "wip"],
    },
    EmojiEntry {
        glyph: "💻",
        name: "laptop",
        aliases: &["computer", "code", "development"],
    },
    EmojiEntry {
        glyph: "🖥️",
        name: "desktop",
        aliases: &["computer", "terminal", "monitor"],
    },
    EmojiEntry {
        glyph: "⌨️",
        name: "keyboard",
        aliases: &["terminal", "typing", "code"],
    },
    EmojiEntry {
        glyph: "💾",
        name: "disk",
        aliases: &["save", "storage", "backup"],
    },
    EmojiEntry {
        glyph: "🔌",
        name: "plug",
        aliases: &["integration", "connect", "power"],
    },
    EmojiEntry {
        glyph: "🔗",
        name: "link",
        aliases: &["connection", "url", "chain"],
    },
    EmojiEntry {
        glyph: "🌐",
        name: "globe",
        aliases: &["web", "internet", "world"],
    },
    EmojiEntry {
        glyph: "🛰️",
        name: "satellite",
        aliases: &["network", "signal", "space"],
    },
    EmojiEntry {
        glyph: "📡",
        name: "antenna",
        aliases: &["network", "signal", "wireless"],
    },
    EmojiEntry {
        glyph: "🛡️",
        name: "shield",
        aliases: &["security", "protect", "defense"],
    },
    EmojiEntry {
        glyph: "🔐",
        name: "secure",
        aliases: &["lock", "key", "privacy"],
    },
    EmojiEntry {
        glyph: "🪪",
        name: "identity",
        aliases: &["id", "auth", "account"],
    },
    EmojiEntry {
        glyph: "👤",
        name: "person",
        aliases: &["user", "profile", "account"],
    },
    EmojiEntry {
        glyph: "👥",
        name: "people",
        aliases: &["team", "users", "community"],
    },
    EmojiEntry {
        glyph: "🤝",
        name: "handshake",
        aliases: &["partner", "deal", "agreement"],
    },
    EmojiEntry {
        glyph: "💬",
        name: "chat",
        aliases: &["message", "conversation", "support"],
    },
    EmojiEntry {
        glyph: "📣",
        name: "megaphone",
        aliases: &["announcement", "marketing", "broadcast"],
    },
    EmojiEntry {
        glyph: "🎯",
        name: "target",
        aliases: &["goal", "focus", "objective"],
    },
    EmojiEntry {
        glyph: "🏁",
        name: "finish",
        aliases: &["flag", "done", "milestone"],
    },
    EmojiEntry {
        glyph: "📌",
        name: "pin",
        aliases: &["location", "important", "bookmark"],
    },
    EmojiEntry {
        glyph: "🗺️",
        name: "map",
        aliases: &["roadmap", "location", "plan"],
    },
    EmojiEntry {
        glyph: "🧭",
        name: "compass",
        aliases: &["direction", "navigate", "guide"],
    },
    EmojiEntry {
        glyph: "📝",
        name: "memo",
        aliases: &["note", "write", "document"],
    },
    EmojiEntry {
        glyph: "📋",
        name: "clipboard",
        aliases: &["task", "list", "paste"],
    },
    EmojiEntry {
        glyph: "📁",
        name: "folder",
        aliases: &["directory", "files", "project"],
    },
    EmojiEntry {
        glyph: "🗂️",
        name: "folders",
        aliases: &["archive", "organize", "projects"],
    },
    EmojiEntry {
        glyph: "🗃️",
        name: "archive",
        aliases: &["storage", "history", "box"],
    },
    EmojiEntry {
        glyph: "📊",
        name: "bar chart",
        aliases: &["analytics", "report", "metrics"],
    },
    EmojiEntry {
        glyph: "🧮",
        name: "calculator",
        aliases: &["math", "finance", "numbers"],
    },
    EmojiEntry {
        glyph: "💳",
        name: "credit card",
        aliases: &["payment", "billing", "money"],
    },
    EmojiEntry {
        glyph: "🏦",
        name: "bank",
        aliases: &["finance", "money", "institution"],
    },
    EmojiEntry {
        glyph: "🏢",
        name: "office",
        aliases: &["business", "company", "work"],
    },
    EmojiEntry {
        glyph: "🏭",
        name: "factory",
        aliases: &["industry", "production", "manufacturing"],
    },
    EmojiEntry {
        glyph: "🛒",
        name: "cart",
        aliases: &["commerce", "shop", "store"],
    },
    EmojiEntry {
        glyph: "🧾",
        name: "receipt",
        aliases: &["invoice", "billing", "expense"],
    },
    EmojiEntry {
        glyph: "🎨",
        name: "palette",
        aliases: &["design", "creative", "ui"],
    },
    EmojiEntry {
        glyph: "✨",
        name: "sparkles",
        aliases: &["magic", "new", "polish"],
    },
    EmojiEntry {
        glyph: "🎵",
        name: "music",
        aliases: &["audio", "sound", "song"],
    },
    EmojiEntry {
        glyph: "🎬",
        name: "movie",
        aliases: &["video", "media", "film"],
    },
    EmojiEntry {
        glyph: "📷",
        name: "camera",
        aliases: &["photo", "image", "media"],
    },
    EmojiEntry {
        glyph: "🎮",
        name: "game",
        aliases: &["gaming", "controller", "play"],
    },
    EmojiEntry {
        glyph: "🏆",
        name: "trophy",
        aliases: &["win", "award", "success"],
    },
    EmojiEntry {
        glyph: "💎",
        name: "gem",
        aliases: &["diamond", "premium", "ruby"],
    },
    EmojiEntry {
        glyph: "🌱",
        name: "seedling",
        aliases: &["growth", "new", "green"],
    },
    EmojiEntry {
        glyph: "🌳",
        name: "tree",
        aliases: &["nature", "growth", "branch"],
    },
    EmojiEntry {
        glyph: "🌊",
        name: "wave",
        aliases: &["water", "flow", "ocean"],
    },
    EmojiEntry {
        glyph: "☀️",
        name: "sun",
        aliases: &["light", "day", "bright"],
    },
    EmojiEntry {
        glyph: "🌙",
        name: "moon",
        aliases: &["night", "dark", "sleep"],
    },
    EmojiEntry {
        glyph: "🌍",
        name: "earth",
        aliases: &["world", "global", "planet"],
    },
    EmojiEntry {
        glyph: "⚡",
        name: "lightning",
        aliases: &["fast", "power", "energy"],
    },
    EmojiEntry {
        glyph: "🍎",
        name: "apple",
        aliases: &["mac", "food", "fruit"],
    },
    EmojiEntry {
        glyph: "🐍",
        name: "snake",
        aliases: &["python", "language", "code"],
    },
    EmojiEntry {
        glyph: "🦀",
        name: "crab",
        aliases: &["rust", "language", "ferris"],
    },
    EmojiEntry {
        glyph: "🐳",
        name: "whale",
        aliases: &["docker", "container", "ocean"],
    },
    EmojiEntry {
        glyph: "🟢",
        name: "green circle",
        aliases: &["status", "online", "go"],
    },
    EmojiEntry {
        glyph: "🟡",
        name: "yellow circle",
        aliases: &["status", "waiting", "warning"],
    },
    EmojiEntry {
        glyph: "🟠",
        name: "orange circle",
        aliases: &["status", "attention", "warm"],
    },
    EmojiEntry {
        glyph: "🔴",
        name: "red circle",
        aliases: &["status", "stop", "error"],
    },
    EmojiEntry {
        glyph: "🔵",
        name: "blue circle",
        aliases: &["status", "info", "cool"],
    },
    EmojiEntry {
        glyph: "🟣",
        name: "purple circle",
        aliases: &["status", "violet", "color"],
    },
    EmojiEntry {
        glyph: "⚫",
        name: "black circle",
        aliases: &["status", "dark", "color"],
    },
    EmojiEntry {
        glyph: "⚪",
        name: "white circle",
        aliases: &["status", "light", "color"],
    },
    EmojiEntry {
        glyph: "🟩",
        name: "green square",
        aliases: &["status", "success", "color"],
    },
    EmojiEntry {
        glyph: "🟨",
        name: "yellow square",
        aliases: &["status", "waiting", "color"],
    },
    EmojiEntry {
        glyph: "🟧",
        name: "orange square",
        aliases: &["status", "attention", "color"],
    },
    EmojiEntry {
        glyph: "🟥",
        name: "red square",
        aliases: &["status", "error", "color"],
    },
    EmojiEntry {
        glyph: "🟦",
        name: "blue square",
        aliases: &["status", "info", "color"],
    },
    EmojiEntry {
        glyph: "🟪",
        name: "purple square",
        aliases: &["status", "violet", "color"],
    },
    EmojiEntry {
        glyph: "⬛",
        name: "black square",
        aliases: &["status", "dark", "color"],
    },
    EmojiEntry {
        glyph: "⬜",
        name: "white square",
        aliases: &["status", "light", "color"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_stays_curated_and_searchable() {
        assert!((80..=100).contains(&CATALOG.len()), "{}", CATALOG.len());
        assert!(CATALOG
            .iter()
            .all(|entry| entry.name == entry.name.to_lowercase()));
        assert!(CATALOG
            .iter()
            .find(|entry| entry.name == "rocket")
            .is_some_and(|entry| entry.matches("deploy")));
    }
}
