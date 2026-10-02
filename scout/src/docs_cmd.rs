//! `scout docs` — Scout Monitoring framework setup documentation links.

pub struct Framework {
    pub name: &'static str,
    pub description: &'static str,
    pub docs_url: &'static str,
}

pub const FRAMEWORKS: &[Framework] = &[
    Framework {
        name: "rails",
        description: "Ruby on Rails",
        docs_url: "https://scoutapm.com/docs/ruby",
    },
    Framework {
        name: "django",
        description: "Python Django",
        docs_url: "https://scoutapm.com/docs/python/django",
    },
    Framework {
        name: "flask",
        description: "Python Flask",
        docs_url: "https://scoutapm.com/docs/python/flask",
    },
    Framework {
        name: "phoenix",
        description: "Elixir Phoenix",
        docs_url: "https://scoutapm.com/docs/elixir",
    },
    Framework {
        name: "express",
        description: "Node.js Express",
        docs_url: "https://scoutapm.com/docs/node/express",
    },
    Framework {
        name: "laravel",
        description: "PHP Laravel",
        docs_url: "https://scoutapm.com/docs/php/laravel",
    },
    Framework {
        name: "sinatra",
        description: "Ruby Sinatra",
        docs_url: "https://scoutapm.com/docs/ruby/sinatra",
    },
    Framework {
        name: "fastapi",
        description: "Python FastAPI",
        docs_url: "https://scoutapm.com/docs/python/fastapi",
    },
    Framework {
        name: "celery",
        description: "Python Celery",
        docs_url: "https://scoutapm.com/docs/python/celery",
    },
    Framework {
        name: "dramatiq",
        description: "Python Dramatiq",
        docs_url: "https://scoutapm.com/docs/python/other-libraries#dramatiq",
    },
    Framework {
        name: "sidekiq",
        description: "Ruby Sidekiq",
        docs_url: "https://scoutapm.com/docs/ruby#instrumented-libraries",
    },
];

pub fn lookup(name: &str) -> Option<&'static Framework> {
    FRAMEWORKS
        .iter()
        .find(|framework| framework.name.eq_ignore_ascii_case(name))
}

pub fn run(framework: Option<&str>, json: bool) -> Result<(), String> {
    match framework {
        None => {
            if json {
                let names: Vec<&str> = FRAMEWORKS.iter().map(|f| f.name).collect();
                println!("{}", serde_json::json!({ "frameworks": names }));
            } else {
                println!("Supported frameworks");
                for framework in FRAMEWORKS {
                    println!("  {:12}  {}", framework.name, framework.description);
                }
                println!();
                println!("Usage: scout docs <framework>");
            }
            Ok(())
        }
        Some(name) => {
            let framework = lookup(name).ok_or_else(|| {
                format!(
                    "unknown framework {name:?} — run 'scout docs' with no arguments to list the supported names"
                )
            })?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "framework": framework.name,
                        "docs_url": framework.docs_url,
                    })
                );
            } else {
                println!("Setup: {}", framework.name);
                println!();
                println!("  Documentation: {}", framework.docs_url);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(lookup("Rails").unwrap().name, "rails");
    }

    #[test]
    fn unknown_framework_errors() {
        assert!(run(Some("nope"), false).is_err());
    }
}
