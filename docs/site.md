# Static site

`oxispec site` turns the specs into a folder of HTML pages, one per node, with no server and no JavaScript framework. Put the folder on any static host (GitHub Pages, S3, an internal web server).

```bash
oxispec site --out public                        # the working tree
oxispec site --out public/v0.2 --at specs-v0.2.0 # the specs as they were at a revision
oxispec site --out public --releases             # latest, and a folder for every spec tag
oxispec site --out public --title "Acme specs"   # default title: the project folder's name
```

## Pages

- **`index.html`** lists the nodes grouped by type, with status, and a filter box that works in the page.
- **One page per node**, named after its ID: `REQ-AUTH-0001.html`. A page shows the type, status, owner, approval and file, the rendered text, the node's links, and **what links here**.
- **Links go by ID, never by path.** A link to `REQ-AUTH-0001.S1` goes to `REQ-AUTH-0001.html#S1`, and scenario headings carry that anchor. IDs written in the text of a node become links too, except in code. A link to a node that does not exist is shown struck through, and a link to another project is shown as plain text.
- **Styling** is one small `style.css` next to the pages. It follows the reader's light or dark setting and works on a phone.

## Versions

A build is one revision, and every link stays inside it, so a reader never lands on another version by following a link. `--at` takes anything `show --at` does: a branch, tag, hash, `date:YYYY-MM-DD`, or a release name without its `specs-` prefix.

`--releases` builds `latest/` (the working tree) and one folder for each spec tag, such as `v0.1.0/`, plus a top `index.html` that redirects to `latest/` and lists the versions. Every page gets a **version picker** that goes to the same ID in the chosen version, or to that version's index when the node did not exist then. (The check uses a `HEAD` request, so it needs a web server. Opened from disk, the picker goes straight to the page.) Each page also shows what changed **since the release before**, the same list `oxispec diff` gives for that node, or "new in this version".

Tags that are versions (`v1.2.3`) are ordered by version. Other tags are ordered by how much history leads up to them. The `--releases` build has no pretty URL scheme like `/p/project/ID@rev` from the design. The folder name plays that role: `public/v0.2.0/REQ-AUTH-0001.html`.

## Safety

Specs are written by many people, and pull requests are not always trusted, so the site treats the text as data. Raw HTML in a node is shown as text, never run. Links whose scheme could run script (`javascript:`, `data:` and so on) are dropped, and only `http`, `https`, `mailto` and relative links stay. Titles and attributes are escaped.

## Rebuilding

A build removes the `.html` files and `style.css` of the folder it writes, and nothing else, so pages of deleted nodes go away and files such as `CNAME` stay. `--out` cannot be the project folder.

Measured on 20,000 requirement files (80 MB of text, macOS, release build):

| | Time |
|---|---|
| `site` of the working tree | 6.3 s (20,002 files, 81 MB) |
| `site --at` a revision | 6.4 s |
| `site --releases` with one tag | 16 s |

## Not built yet

Search on the site (it needs the index shipped with the pages), a diagram viewer, a trace panel (code and tests, after the scanners), a diff page between any two versions, a per-project URL scheme for several projects, and access control. The server mode in the design serves these pages dynamically with access rules.
