extern crate ansi_term;
extern crate clap;
extern crate git2;

use ansi_term::Colour::{Blue, Fixed, Green, Red, White};
use clap::{App, Arg};
use git2::{Repository, Status, StatusEntry};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Components, Path};

#[derive(Debug)]
enum Node {
    Tree(Tree),
    Leaf(Leaf),
}

#[derive(Debug)]
struct Tree {
    name: String,
    children: BTreeMap<String, Node>,
}

#[derive(Debug)]
struct Leaf {
    name: String,
    status: Status,
}

impl Tree {
    fn add_entry(&mut self, entry: &StatusEntry) {
        entry
            .path()
            .and_then(|path| Path::new(path).parent())
            .map(|parent| {
                self.add_entry_at_path(entry, &mut parent.components());
            });
    }

    fn add_entry_at_path(&mut self, entry: &StatusEntry, path: &mut Components) {
        match path.next() {
            Some(Component::Normal(ref dir)) => {
                dir.to_str().map(|dir| {
                    let node = self.children.entry(dir.into()).or_insert(Node::Tree(Tree {
                        name: dir.into(),
                        children: BTreeMap::new(),
                    }));

                    if let &mut Node::Tree(ref mut node) = node {
                        node.add_entry_at_path(entry, path)
                    }
                });
            }

            Some(_) => unimplemented!(),

            _ => {
                entry
                    .path()
                    .map(|path| Path::new(path))
                    .and_then(|path| path.file_name())
                    .and_then(|file_name| file_name.to_str())
                    .map(|file_name| {
                        let new_node = Leaf {
                            name: file_name.into(),
                            status: entry.status(),
                        };

                        self.children.insert(file_name.into(), Node::Leaf(new_node))
                    });
            }
        }
    }
}

trait Lines {
    fn lines(&self) -> Vec<String>;

    fn prepend(&self, lines: Vec<String>, with: String) -> Vec<String> {
        lines
            .into_iter()
            .map(|line| {
                let mut new_line = with.clone();
                new_line.push_str(&line);
                new_line
            })
            .collect()
    }

    fn prepend_first_and_rest(
        &self,
        mut lines: Vec<String>,
        first_with: String,
        rest_with: String,
    ) -> Vec<String> {
        let (first, rest) = lines.split_at_mut(1);

        let mut first_line = first_with.clone();
        first_line.push_str(&first[0]);

        let mut lines = vec![first_line];

        lines.extend(self.prepend(rest.into(), rest_with));

        lines
    }
}

impl Lines for Node {
    fn lines(&self) -> Vec<String> {
        match self {
            &Node::Tree(ref node) => node.lines(),
            &Node::Leaf(ref node) => node.lines(),
        }
    }
}

impl Lines for Tree {
    fn lines(&self) -> Vec<String> {
        let children = self.children.values().collect::<Vec<_>>();

        let split_at = match children.len() {
            0 => 0,
            len => len - 1,
        };

        let (rest, last) = children.as_slice().split_at(split_at);

        let mut lines = vec![self.name.clone()];

        lines.extend(
            rest.iter()
                .flat_map(|&node| {
                    // Every child’s first line gets prepended by "├── ".
                    // All following lines get prepended by "│   ".

                    self.prepend_first_and_rest(node.lines(), "├── ".into(), "│   ".into())
                })
                .collect::<Vec<_>>(),
        );

        if let Some(&last) = last.get(0) {
            // The last child’s first line gets prepended by "└── ".
            // All following lines get prepended by "    ".
            lines.extend(self.prepend_first_and_rest(
                last.lines(),
                "└── ".into(),
                "    ".into(),
            ));
        }

        lines
    }
}

// http://www.calmar.ws/vim/256-xterm-24bit-rgb-color-chart.html
impl Lines for Leaf {
    fn lines(&self) -> Vec<String> {
        let style = match self.status {
            s if s.contains(git2::STATUS_WT_MODIFIED) => Red.normal(),
            s if s.contains(git2::STATUS_INDEX_MODIFIED) => Red.bold(),
            s if s.contains(git2::STATUS_WT_NEW) => Green.normal(),
            s if s.contains(git2::STATUS_INDEX_NEW) => Green.bold(),
            s if s.contains(git2::STATUS_IGNORED) => Blue.normal(),
            _ => White.normal(),
        };

        let modifier_index = match self.status {
            s if s.contains(git2::STATUS_INDEX_MODIFIED) => "M",
            s if s.contains(git2::STATUS_INDEX_NEW) => "N",
            _ => "-",
        };

        let modifier_worktree = match self.status {
            s if s.contains(git2::STATUS_WT_MODIFIED) => "M",
            s if s.contains(git2::STATUS_WT_NEW) => "N",
            _ => "-",
        };

        let gray = Fixed(244).normal();

        vec![
            format!(
                "{}{} {}",
                gray.paint(modifier_index),
                gray.paint(modifier_worktree),
                style.paint(self.name.as_str())
            ),
        ]
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for l in self.lines() {
            f.write_str(&l)?;
            f.write_str("\n")?;
        }

        Ok(())
    }
}

fn main() {
    let matches = App::new("git-tree")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Christoph Rüßler <christoph.ruessler@mailbox.org>")
        .about("tree + git status: displays git status info in a tree")
        .after_help(
            "git-tree searches for a git repository the same way git does, and \
             displays a tree showing untracked and modified files. The tree’s \
             root is the repository’s root. The tree’s items are colored to \
             indicate their status (green: new, red: modified, blue: ignored). \
             Changes to files in the index are shown in bold.\n\
             A column in front of each file’s name indicates changes to the \
             index and the working tree, respectively (M: modified, N: new).",
        )
        .arg(
            Arg::with_name("all")
                .short("a")
                .long("all")
                .help("Include ignored files"),
        )
        .get_matches();

    let repo = Repository::discover("./").expect("Could not open repository");
    let statuses = repo.statuses(None).expect("Could not get statuses");

    let mut root = Tree {
        name: ".".into(),
        children: BTreeMap::new(),
    };

    for s in statuses.iter() {
        if matches.is_present("all") || !s.status().contains(git2::STATUS_IGNORED) {
            root.add_entry(&s);
        }
    }

    println!("{}", root);
}
