use ansi_term::Colour::{Blue, Fixed, Green, Red, White, Yellow};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use git2::Repository;
use gix::bstr::{BStr, ByteSlice};
use gix::ObjectId;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::ReadDir;
use std::path::{Component, Components, Path};

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

// TODO:
// Either rename some variants to match `git2::Status` variants or add docs on how they are
// related.
#[derive(Debug)]
enum Status {
    WorktreeRemoved,
    WorktreeAdded,
    WorktreeModified,
    IndexRemoved,
    IndexAdded,
    IndexModified,
    TypeChange,
    Renamed,
    Copied,
    IntentToAdd,
    Conflict,

    // TODO:
    // Is this the correct name? This is currently used to reflect `item.summary(): Option<Status>
    // == None`.
    //
    // We might want to consider moving this out of status at some point, converting calling code
    // to `Option<Status>` if that makes sense.
    Ignored,
}

impl From<gix::status::Item> for Status {
    fn from(item: gix::status::Item) -> Self {
        use gix::diff::index::ChangeRef;
        use gix::status::index_worktree::iter::Summary;

        match item {
            gix::status::Item::IndexWorktree(item) => match item.summary() {
                Some(summary) => match summary {
                    Summary::Removed => Self::WorktreeRemoved,
                    Summary::Added => Self::WorktreeAdded,
                    Summary::Modified => Self::WorktreeModified,
                    Summary::TypeChange => Self::TypeChange,
                    Summary::Renamed => Self::Renamed,
                    Summary::Copied => Self::Copied,
                    Summary::IntentToAdd => Self::IntentToAdd,
                    Summary::Conflict => Self::Conflict,
                },
                None => Self::Ignored,
            },
            gix::status::Item::TreeIndex(change_ref) => match change_ref {
                ChangeRef::Addition { .. } => Self::IndexAdded,
                ChangeRef::Deletion { .. } => Self::IndexRemoved,
                ChangeRef::Modification { .. } => Self::IndexModified,
                ChangeRef::Rewrite { .. } => Self::IndexModified,
            },
        }
    }
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

fn calculate_stats(
    hash_kind: gix::hash::Kind,
    resource_cache: &mut gix::diff::blob::Platform,
    objects: &impl gix::objs::FindObjectOrHeader,
    previous_id: Option<ObjectId>,
    id: Option<ObjectId>,
    path: &BStr,
    diff_stat: &mut DiffStat,
) -> Result<()> {
    resource_cache.set_resource(
        previous_id.unwrap_or_else(|| ObjectId::null(hash_kind)),
        gix::object::tree::EntryKind::Blob,
        path,
        gix::diff::blob::ResourceKind::OldOrSource,
        objects,
    )?;
    resource_cache.set_resource(
        id.unwrap_or_else(|| ObjectId::null(hash_kind)),
        gix::object::tree::EntryKind::Blob,
        path,
        gix::diff::blob::ResourceKind::NewOrDestination,
        objects,
    )?;

    let outcome = resource_cache.prepare_diff()?;
    let input = gix::diff::blob::intern::InternedInput::new(
        outcome.old.data.as_slice().unwrap_or_default(),
        outcome.new.data.as_slice().unwrap_or_default(),
    );

    let counter = gix::diff::blob::diff(
        gix::diff::blob::Algorithm::Histogram,
        &input,
        gix::diff::blob::sink::Counter::default(),
    );

    diff_stat.files_changed += 1;
    diff_stat.insertions += counter.insertions as usize;
    diff_stat.deletions += counter.removals as usize;

    Ok(())
}

impl TryFrom<gix::Repository> for DiffStat {
    type Error = anyhow::Error;

    fn try_from(repo: gix::Repository) -> std::result::Result<Self, Self::Error> {
        let branch: OsString = match repo.head_name()? {
            Some(name) => name
                .shorten()
                .to_os_str()
                .to_owned()
                .context("HEAD is not a direct reference")?
                .into(),
            None => "detached HEAD".into(),
        };

        let worktree_roots = gix::diff::blob::pipeline::WorktreeRoots {
            old_root: None,
            new_root: repo.workdir().map(ToOwned::to_owned),
        };

        let mut resource_cache = repo.diff_resource_cache(
            gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent,
            worktree_roots,
        )?;

        let mut diff_stat = DiffStat {
            branch,
            files_changed: 0,
            insertions: 0,
            deletions: 0,
        };

        let status = repo
            .status(gix::progress::Discard)?
            .untracked_files(gix::status::UntrackedFiles::None);
        let iter = status.into_iter(None)?;

        for item in iter {
            let item = item?;

            // TODO:
            // Check the implementation of `Diff::stats` to figure out what `git` does in each of
            // the other cases. Add more tests in case some cases aren't covered yet.
            match item {
                gix::status::Item::IndexWorktree(item) => {
                    // This yields changes that have not been staged yet.
                    use gix::status::index_worktree::Item;

                    match item {
                        Item::Modification {
                            entry, rela_path, ..
                        } => {
                            calculate_stats(
                                repo.object_hash(),
                                &mut resource_cache,
                                &repo.objects,
                                Some(entry.id),
                                None,
                                rela_path.as_ref(),
                                &mut diff_stat,
                            )?;
                        }
                        Item::DirectoryContents { .. } => {
                            // TODO:
                            // Double-check that this is what `git2` does.
                            // Do nothing.
                        }
                        Item::Rewrite { .. } => {
                            // TODO:
                            // Double-check that this is what `git2` does.
                            // Do nothing.
                        }
                    };
                }
                gix::status::Item::TreeIndex(change_ref) => {
                    // This yields changes that have already been staged.
                    use gix::diff::index::ChangeRef;

                    match change_ref {
                        ChangeRef::Addition { .. } => {
                            // TODO:
                            // Double-check that this is what `git2` does.
                            // Do nothing.
                        }
                        ChangeRef::Deletion { location, id, .. } => {
                            calculate_stats(
                                repo.object_hash(),
                                &mut resource_cache,
                                &repo.objects,
                                Some(id.into_owned()),
                                None,
                                &location,
                                &mut diff_stat,
                            )?;
                        }
                        ChangeRef::Modification {
                            location,
                            previous_id,
                            id,
                            ..
                        } => {
                            calculate_stats(
                                repo.object_hash(),
                                &mut resource_cache,
                                &repo.objects,
                                Some(previous_id.into_owned()),
                                Some(id.into_owned()),
                                &location,
                                &mut diff_stat,
                            )?;
                        }
                        ChangeRef::Rewrite { .. } => {
                            // TODO:
                            // Double-check that this is what `git2` does.
                            // Do nothing.
                        }
                    };
                }
            };
        }

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
            Some(Component::Normal(dir)) => {
                if let Some(dir) = dir.to_str() {
                    let new_node = self.children.entry(dir.into()).or_insert(Node::Tree(Tree {
                        name: dir.into(),
                        children: BTreeMap::new(),
                    }));

                    if let &mut Node::Tree(ref mut new_node) = new_node {
                        new_node.add_node_at_path(node, name, path)
                    }
                };
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
            Node::Tree(node) => node.lines(),
            Node::Summary(node) => node.lines(),
            Node::Leaf(node) => node.lines(),
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
                    // Every child's first line gets prepended by "├── ".
                    // All following lines get prepended by "│   ".
                    self.prepend_first_and_rest(node.lines(), "├── ".into(), "│   ".into())
                })
                .collect::<Vec<_>>(),
        );

        if let Some(&last) = last.first() {
            // The last child's first line gets prepended by "└── ".
            // All following lines get prepended by "    ".
            lines.extend(self.prepend_first_and_rest(last.lines(), "└── ".into(), "    ".into()));
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
            Status::WorktreeModified => Red.normal(),
            Status::IndexModified => Red.bold(),
            Status::WorktreeAdded => Green.normal(),
            Status::IndexAdded => Green.bold(),
            Status::Ignored => Blue.normal(),
            _ => White.normal(),
        };

        let modifier_index = match self.status {
            Status::IndexModified => "M",
            Status::IndexAdded => "N",
            Status::IndexRemoved => "D",
            _ => "-",
        };

        let modifier_worktree = match self.status {
            Status::WorktreeModified => "M",
            Status::WorktreeAdded => "N",
            // TODO:
            // Mention "D" in help.
            Status::WorktreeRemoved => "D",
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
            writeln!(f, "{}", l.as_os_str().to_string_lossy())?;
        }

        Ok(())
    }
}

fn walk_repository(repo: &Repository, name: &OsStr, args: &Args) -> Result<Option<Node>> {
    if args.summary {
        walk_summary(repo, name, args)
    } else {
        walk_entries(repo, name, args)
    }
}

fn walk_entries(repo: &Repository, name: &OsStr, args: &Args) -> Result<Option<Node>> {
    let repo = gix::open(
        repo.workdir()
            .ok_or(anyhow!("`Repository::workdir()` returned `None`"))?,
    )?;
    let status = repo.status(gix::progress::Discard)?;

    let mut root = Tree {
        name: name.into(),
        children: BTreeMap::new(),
    };

    for item in status.into_iter(Vec::new())? {
        let item = item?;
        let status = item.clone().into();

        if args.all || !matches!(status, Status::Ignored) {
            let path = Path::new(item.location().to_os_str()?);
            let file_name = file_name(path);

            let parent_path = path.parent();

            if let Some(parent_path) = parent_path {
                let leaf = Leaf {
                    name: file_name.into(),
                    status: item.clone().into(),
                };

                root.add_leaf_at_path(leaf, &mut parent_path.components());
            }
        }
    }

    Ok(Some(Node::Tree(root)))
}

fn file_name(path: &Path) -> &OsStr {
    path.file_name().unwrap_or(path.as_os_str())
}

fn walk_summary(repo: &Repository, name: &OsStr, args: &Args) -> Result<Option<Node>> {
    let repo = gix::open(
        repo.workdir()
            .ok_or(anyhow!("`Repository::workdir()` returned `None`"))?,
    )?;
    let stats: DiffStat = repo.try_into()?;

    if args.only_show_changes && stats.insertions == 0 && stats.deletions == 0 {
        return Ok(None);
    }

    let summary = Summary {
        name: name.into(),
        stats,
    };

    Ok(Some(Node::Summary(summary)))
}

fn walk_directory(path: &Path, iter: ReadDir, depth: usize, args: &Args) -> Result<Node> {
    let mut tree = Tree {
        name: file_name(path).into(),
        children: BTreeMap::new(),
    };

    let directories = iter.filter_map(|e| e.ok()).collect::<Vec<_>>();

    let new_entries = directories
        .iter()
        .filter_map(|entry| {
            walk_path(&entry.path(), depth - 1, args)
                .ok()
                .and_then(|child| child.map(|child| (child, entry.file_name())))
        })
        .collect::<Vec<(Node, OsString)>>();

    for (node, file_name) in new_entries {
        tree.add_node(node, file_name)
    }

    Ok(Node::Tree(tree))
}

fn walk_path(path: &Path, depth: usize, args: &Args) -> Result<Option<Node>> {
    if path.is_dir() {
        match Repository::open(path) {
            Ok(repo) => {
                let node = walk_repository(&repo, file_name(path), args)?;

                Ok(node)
            }

            _ => {
                if depth > 0 {
                    let node = walk_directory(path, path.read_dir()?, depth, args)?;

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

fn fallback(path: &Path, args: &Args) -> Result<Option<Node>> {
    let repo = match Repository::discover(path) {
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

    walk_repository(&repo, file_name(path), args)
}

#[derive(Parser, Debug)]
/// tree + git status: displays git status info in a tree
///
/// git-tree searches for a git repository the same way git does, and displays
/// a tree showing untracked and modified files. The tree's root is the
/// repository's root. The tree's items are colored to indicate their status
/// (green: new, red: modified, blue: ignored). Changes to files in the index
/// are shown in bold.
///
/// A column in front of each file's name indicates changes to the index and
/// the working tree, respectively (M: modified, N: new).
#[command(author, version, about)]
struct Args {
    /// Include ignored files
    #[arg(short, long)]
    all: bool,

    /// Recursively search for repositories up to <depth> levels deep
    #[arg(long, default_value = "0")]
    depth: usize,

    /// Show only a summary containing the number of additions, deletions, and
    /// changed files
    #[arg(short, long)]
    summary: bool,

    /// Only show repositories that contains changes (useful in combination
    /// with --depth and --summary)
    #[arg(long)]
    only_show_changes: bool,
}

fn run() -> Result<()> {
    let args = Args::parse();

    let path = Path::new(".");

    let node = match walk_path(path, args.depth, &args)? {
        node @ Some(_) => node,
        None => fallback(path, &args)?,
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
