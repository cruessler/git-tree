use ansi_term::Colour::{Blue, Fixed, Green, Red, White, Yellow};
use anyhow::{anyhow, Result};
use clap::{App, Arg};
use git2::{Branch, Repository, Status};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
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
    name: OsString,
    children: BTreeMap<OsString, Node>,
}

#[derive(Debug)]
struct Summary {
    name: OsString,
    stats: DiffStat,
}

#[derive(Debug)]
struct Leaf {
    name: OsString,
    status: Status,
}

#[derive(Debug)]
struct DiffStat {
    branch: OsString,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
}

impl DiffStat {
    fn from(repo: &Repository) -> Result<DiffStat> {
        let head = repo.head()?;
        let object_id = head
            .target()
            .ok_or(anyhow!("HEAD is not a direct reference"))?;

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
    fn add_leaf_at_path(&mut self, leaf: Leaf, path: &mut Components<'_>) {
        let name = leaf.name.clone();

        self.add_node_at_path(Node::Leaf(leaf), name, path);
    }

    fn add_node_at_path(&mut self, node: Node, name: OsString, path: &mut Components<'_>) {
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

    fn add_node(&mut self, node: Node, name: OsString) {
        self.children.insert(name, node);
    }
}

trait Lines {
    fn lines(&self) -> Vec<OsString>;

    fn prepend(&self, lines: Vec<OsString>, with: OsString) -> Vec<OsString> {
        lines
            .into_iter()
            .map(|line| {
                let mut new_line = with.clone();
                new_line.push(&line);
                new_line
            })
            .collect()
    }

    fn prepend_first_and_rest(
        &self,
        mut lines: Vec<OsString>,
        first_with: OsString,
        rest_with: OsString,
    ) -> Vec<OsString> {
        let (first, rest) = lines.split_at_mut(1);

        let mut first_line = first_with.clone();
        first_line.push(&first[0]);

        let mut lines = vec![first_line];

        lines.extend(self.prepend(rest.into(), rest_with));

        lines
    }
}

impl Lines for Node {
    fn lines(&self) -> Vec<OsString> {
        match self {
            &Node::Tree(ref node) => node.lines(),
            &Node::Summary(ref node) => node.lines(),
            &Node::Leaf(ref node) => node.lines(),
        }
    }
}

impl Lines for Tree {
    fn lines(&self) -> Vec<OsString> {
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
    fn lines(&self) -> Vec<OsString> {
        vec![format!(
            "{} {} +{} -{} ({})",
            self.name.as_os_str().to_string_lossy(),
            Fixed(244).paint(format!(
                "[{}]",
                self.stats.branch.as_os_str().to_string_lossy()
            )),
            Green.paint(format!("{}", self.stats.insertions)),
            Red.paint(format!("{}", self.stats.deletions)),
            Yellow.paint(format!("{}", self.stats.files_changed)),
        )
        .into()]
    }
}

// http://www.calmar.ws/vim/256-xterm-24bit-rgb-color-chart.html
impl Lines for Leaf {
    fn lines(&self) -> Vec<OsString> {
        let style = match self.status {
            s if s.contains(git2::Status::WT_MODIFIED) => Red.normal(),
            s if s.contains(git2::Status::INDEX_MODIFIED) => Red.bold(),
            s if s.contains(git2::Status::WT_NEW) => Green.normal(),
            s if s.contains(git2::Status::INDEX_NEW) => Green.bold(),
            s if s.contains(git2::Status::IGNORED) => Blue.normal(),
            _ => White.normal(),
        };

        let modifier_index = match self.status {
            s if s.contains(git2::Status::INDEX_MODIFIED) => "M",
            s if s.contains(git2::Status::INDEX_NEW) => "N",
            _ => "-",
        };

        let modifier_worktree = match self.status {
            s if s.contains(git2::Status::WT_MODIFIED) => "M",
            s if s.contains(git2::Status::WT_NEW) => "N",
            _ => "-",
        };

        let gray = Fixed(244).normal();

        vec![format!(
            "{}{} {}",
            gray.paint(modifier_index),
            gray.paint(modifier_worktree),
            style.paint(format!("{}", self.name.as_os_str().to_string_lossy()))
        )
        .into()]
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for l in self.lines() {
            write!(f, "{}\n", l.as_os_str().to_string_lossy())?;
        }

        Ok(())
    }
}

struct Flags<'a> {
    all: bool,
    only_show_changes: bool,
    depth: Option<&'a str>,
    summary: bool,
}

fn walk_repository(repo: &Repository, name: &OsStr, flags: &Flags<'_>) -> Result<Option<Node>> {
    if flags.summary {
        walk_summary(&repo, name, &flags)
    } else {
        walk_entries(&repo, name, &flags)
    }
}

fn walk_entries(repo: &Repository, name: &OsStr, flags: &Flags<'_>) -> Result<Option<Node>> {
    let statuses = repo.statuses(None)?;

    let mut root = Tree {
        name: name.into(),
        children: BTreeMap::new(),
    };

    for entry in statuses.iter() {
        if flags.all || !entry.status().contains(git2::Status::IGNORED) {
            let path = Path::new(
                entry
                    .path()
                    .ok_or(anyhow!("{:?} cannot be resolved to a path", entry.path()))?,
            );

            let file_name = file_name(&path);

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

    Ok(Some(Node::Tree(root)))
}

fn file_name(path: &Path) -> &OsStr {
    path.file_name().unwrap_or_else(|| path.as_os_str())
}

fn walk_summary(repo: &Repository, name: &OsStr, flags: &Flags<'_>) -> Result<Option<Node>> {
    let stats = DiffStat::from(repo)?;

    if flags.only_show_changes && stats.insertions == 0 && stats.deletions == 0 {
        return Ok(None);
    }

    let summary = Summary {
        name: name.into(),
        stats: stats,
    };

    return Ok(Some(Node::Summary(summary)));
}

fn walk_directory(path: &Path, iter: ReadDir, depth: usize, flags: &Flags<'_>) -> Result<Node> {
    let mut tree = Tree {
        name: file_name(path).into(),
        children: BTreeMap::new(),
    };

    let directories = iter.filter_map(|e| e.ok()).collect::<Vec<_>>();

    let new_entries = directories
        .iter()
        .filter_map(|ref entry| {
            walk_path(&entry.path(), depth - 1, &flags)
                .ok()
                .and_then(|child| child.map(|child| (child, entry.file_name())))
        })
        .collect::<Vec<(Node, OsString)>>();

    for (node, file_name) in new_entries {
        tree.add_node(node, file_name)
    }

    Ok(Node::Tree(tree))
}

fn walk_path(path: &Path, depth: usize, flags: &Flags<'_>) -> Result<Option<Node>> {
    if path.is_dir() {
        match Repository::open(&path) {
            Ok(repo) => {
                let node = walk_repository(&repo, file_name(path), &flags)?;

                Ok(node)
            }

            _ => {
                if depth > 0 {
                    let node = walk_directory(&path, path.read_dir()?, depth, &flags)?;

                    Ok(Some(node))
                } else {
                    Ok(None)
                }
            }
        }
    } else {
        Ok(None)
    }
}

fn fallback(path: &Path, flags: &Flags<'_>) -> Result<Option<Node>> {
    let repo = match Repository::discover(&path) {
        Err(ref error)
            if (error.class() == git2::ErrorClass::Repository
                && error.code() == git2::ErrorCode::NotFound) =>
        {
            return Err(anyhow!(
                "no git repository found at {:?}, you might want to try \
                 running git-tree with `--depth`, see `git-tree --help` for \
                 details",
                path
            ));
        }
        otherwise => otherwise?,
    };

    walk_repository(&repo, file_name(path), &flags)
}

fn run() -> Result<()> {
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
        .arg(
            Arg::with_name("only-show-changes")
                .long("only-show-changes")
                .help(
                    "Only show repositories that contains changes (useful in \
                     combination with --depth and --summary)",
                ),
        )
        .get_matches();

    let flags = Flags {
        all: matches.is_present("all"),
        depth: matches.value_of("depth"),
        only_show_changes: matches.is_present("only-show-changes"),
        summary: matches.is_present("summary"),
    };

    let path = Path::new(".");

    let depth = match flags.depth {
        Some(depth) => depth.parse::<usize>()?,
        None => 0,
    };

    let node = match walk_path(&path, depth, &flags)? {
        node @ Some(_) => node,
        None => fallback(&path, &flags)?,
    };

    match node {
        Some(root) => println!("{}", root),
        _ => println!("no git repository found at {:?}", path),
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        println!("{}", err);
    }
}
