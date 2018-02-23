extern crate ansi_term;
extern crate clap;
extern crate failure;
extern crate git2;

use ansi_term::Colour::{Blue, Fixed, Green, Red, White, Yellow};
use clap::{App, Arg};
use failure::Error;
use git2::{Branch, Repository, Status};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::ReadDir;
use std::path::{Component, Components, Path};
use std::str;

#[derive(Debug)]
enum Node {
    Tree(Tree),
    Summary(Summary),
    Leaf(Leaf),
}

#[derive(Debug)]
struct Tree {
    name: String,
    children: BTreeMap<String, Node>,
}

#[derive(Debug)]
struct Summary {
    name: String,
    stats: DiffStat,
}

#[derive(Debug)]
struct Leaf {
    name: String,
    status: Status,
}

#[derive(Debug)]
struct DiffStat {
    branch: String,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
}

impl DiffStat {
    fn from(repo: &Repository) -> Result<DiffStat, Error> {
        let head = repo.head()?;
        let object_id = head.target()
            .ok_or(failure::err_msg("HEAD is not a direct reference"))?;

        let head_commit = repo.find_commit(object_id)?;
        let head_tree = head_commit.tree()?;

        let diff = repo.diff_tree_to_workdir(Some(&head_tree), None)?;
        let stats = diff.stats()?;

        let branch = Branch::wrap(head);
        let branch_name = str::from_utf8(branch.name_bytes()?)?;

        let diff_stat = DiffStat {
            branch: branch_name.into(),
            files_changed: stats.files_changed(),
            insertions: stats.insertions(),
            deletions: stats.deletions(),
        };

        Ok(diff_stat)
    }
}

impl Tree {
    fn add_leaf_at_path(&mut self, leaf: Leaf, path: &mut Components) {
        let name = leaf.name.clone();

        self.add_node_at_path(Node::Leaf(leaf), name.as_str(), path);
    }

    fn add_node_at_path(&mut self, node: Node, name: &str, path: &mut Components) {
        match path.next() {
            Some(Component::Normal(ref dir)) => {
                dir.to_str().map(|dir| {
                    let new_node = self.children.entry(dir.into()).or_insert(Node::Tree(Tree {
                        name: dir.into(),
                        children: BTreeMap::new(),
                    }));

                    if let &mut Node::Tree(ref mut new_node) = new_node {
                        new_node.add_node_at_path(node, name, path)
                    }
                });
            }

            Some(_) => unimplemented!(),

            _ => {
                self.add_node(node, name);
            }
        }
    }

    fn add_node(&mut self, node: Node, name: &str) {
        self.children.insert(name.into(), node);
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
            &Node::Summary(ref node) => node.lines(),
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

impl Lines for Summary {
    fn lines(&self) -> Vec<String> {
        vec![
            format!(
                "{} {} +{} -{} ({})",
                self.name.as_str(),
                Fixed(244).paint(format!("[{}]", self.stats.branch.as_str())),
                Green.paint(format!("{}", self.stats.insertions)),
                Red.paint(format!("{}", self.stats.deletions)),
                Yellow.paint(format!("{}", self.stats.files_changed)),
            ),
        ]
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

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for l in self.lines() {
            f.write_str(&l)?;
            f.write_str("\n")?;
        }

        Ok(())
    }
}

struct Flags<'a> {
    all: bool,
    depth: Option<&'a str>,
    summary: bool,
}

fn walk_repository(repo: &Repository, name: &str, flags: &Flags) -> Result<Node, Error> {
    let statuses = repo.statuses(None)?;

    let mut root = Tree {
        name: name.into(),
        children: BTreeMap::new(),
    };

    for entry in statuses.iter() {
        if flags.all || !entry.status().contains(git2::STATUS_IGNORED) {
            let path = Path::new(entry.path().ok_or(failure::err_msg(format!(
                "{:?} cannot be resolved to a path",
                entry.path()
            )))?);

            let file_name = file_name(&path)?;

            let leaf = Leaf {
                name: file_name.into(),
                status: entry.status(),
            };

            entry
                .path()
                .and_then(|path| Path::new(path).parent())
                .map(|parent| {
                    root.add_leaf_at_path(leaf, &mut parent.components());
                });
        }
    }

    Ok(Node::Tree(root))
}

fn file_name(path: &Path) -> Result<&str, Error> {
    let file_name = match path.file_name() {
        Some(file_name) => file_name.to_str(),
        None => path.to_str(),
    };

    file_name.ok_or(failure::err_msg(format!(
        "{:?} cannot be resolved to a filename",
        path
    )))
}

fn walk_summary(repo: &Repository, name: &str) -> Result<Node, Error> {
    let stats = DiffStat::from(repo)?;

    let summary = Summary {
        name: name.into(),
        stats: stats,
    };

    Ok(Node::Summary(summary))
}

fn walk_directory(path: &Path, iter: ReadDir, depth: usize, flags: &Flags) -> Result<Node, Error> {
    let mut tree = Tree {
        name: String::from(file_name(path)?),
        children: BTreeMap::new(),
    };

    for entry in iter {
        if let Ok(entry) = entry {
            if let Ok(Some(child)) = walk_path(&entry.path(), depth - 1, &flags) {
                tree.add_node(
                    child,
                    entry.file_name().to_str().ok_or(failure::err_msg(format!(
                        "{:?} cannot be resolved to a filename",
                        entry.file_name()
                    )))?,
                );
            }
        }
    }

    Ok(Node::Tree(tree))
}

fn walk_path(path: &Path, depth: usize, flags: &Flags) -> Result<Option<Node>, Error> {
    match Repository::open(&path) {
        Ok(repo) => {
            let node = if flags.summary {
                walk_summary(&repo, file_name(path)?)
            } else {
                walk_repository(&repo, file_name(path)?, &flags)
            };

            node.map(|node| Some(node))
        }

        _ => {
            if path.is_dir() && depth > 0 {
                let iter = path.read_dir()?;

                walk_directory(&path, iter, depth, &flags).and_then(|node| Ok(Some(node)))
            } else {
                Ok(None)
            }
        }
    }
}

fn fallback(path: &Path, flags: &Flags) -> Result<Option<Node>, Error> {
    let repo = Repository::discover(&path)?;

    let node = walk_repository(&repo, file_name(path)?, &flags);

    node.map(|node| Some(node))
}

fn run(path: &Path, flags: &Flags) -> Result<Option<Node>, Error> {
    let depth = match flags.depth {
        Some(depth) => depth.parse::<usize>()?,
        None => 0,
    };

    match walk_path(&path, depth, &flags) {
        Ok(None) => fallback(&path, &flags),
        result => result,
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
        .arg(
            Arg::with_name("depth")
                .long("depth")
                .takes_value(true)
                .help("Recursively search for repositories up to <depth> levels deep"),
        )
        .arg(Arg::with_name("summary").short("s").long("summary").help(
            "Show only a summary containing the number of additions, deletions, \
             and changed files",
        ))
        .get_matches();

    let flags = Flags {
        all: matches.is_present("all"),
        depth: matches.value_of("depth"),
        summary: matches.is_present("summary"),
    };

    let path = Path::new(".");

    match run(&path, &flags) {
        Ok(Some(root)) => println!("{}", root),
        Ok(_) => println!("no git repository found at {:?}", path),
        Err(err) => println!("{}", err),
    }
}
