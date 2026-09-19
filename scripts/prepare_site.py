"""Maintain a private Zola input tree with protected Markdown mathematics."""

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tempfile

from math_markdown import protect_math


INPUT_DIRECTORIES = ('content', 'templates', 'static', 'sass', 'data', 'themes')
EXCLUDED_NAMES = {'public', '.git', 'article-manifest.json'}
ORIGINAL_BODIES = Path('data/original-bodies.json')


def _markdown_body(text):
    """Keep the original Markdown body, excluding TOML or YAML front matter."""
    opening = re.match(r'\A\ufeff?(\+\+\+|---)[ \t]*\r?\n', text)
    if opening:
        closing = re.search(r'^' + re.escape(opening.group(1)) + r'[ \t]*(?:\r?\n|$)',
                            text[opening.end():], re.M)
        if closing:
            return text[opening.end() + closing.end():]
    return text


def _reject_link(path):
    """Reject both ordinary symlinks and Windows junction/reparse points."""
    try:
        info = path.lstat()
    except FileNotFoundError:
        return
    if stat.S_ISLNK(info.st_mode) or (
        getattr(info, 'st_file_attributes', 0)
        & getattr(stat, 'FILE_ATTRIBUTE_REPARSE_POINT', 0x400)
    ):
        raise ValueError(f'Site staging does not accept links or reparse points: {path}')


def _reject_link_parents(path):
    for current in (path, *path.parents):
        _reject_link(current)


class PreparedSite:
    """Mirror approved Zola inputs into an empty, separate staging directory.

    ``sync`` returns relative ``Path`` objects for changed or removed files.
    Only files and directories created by this instance are ever removed;
    generated Zola output in the staging tree is left alone.
    """

    def __init__(self, source: Path, target: Path):
        source = Path(source).absolute()
        target = Path(target).absolute()
        _reject_link_parents(source)
        _reject_link_parents(target)
        self.source = source.resolve()
        self.target = target.resolve()
        if (self.source == self.target
                or self.source in self.target.parents
                or self.target in self.source.parents):
            raise ValueError('Source and staging target must not overlap')
        if not self.source.is_dir():
            raise ValueError(f'Site source is not a directory: {self.source}')
        if self.target.exists() and (
            not self.target.is_dir() or any(self.target.iterdir())
        ):
            raise ValueError(f'Staging target must be an empty directory: {self.target}')
        self.target.mkdir(parents=True, exist_ok=True)
        self._snapshot = {}
        self._directories = set()
        self._body_cache = {}

    def _scan(self):
        """Validate the complete input selection before changing the mirror."""
        _reject_link_parents(self.source)
        files = {}
        directories = set()

        def visit(path):
            if path.name in EXCLUDED_NAMES:
                return
            _reject_link(path)
            relative = path.relative_to(self.source)
            info = path.stat()
            if stat.S_ISDIR(info.st_mode):
                directories.add(relative)
                for child in sorted(path.iterdir()):
                    visit(child)
            elif stat.S_ISREG(info.st_mode):
                files[relative] = (info.st_mtime_ns, info.st_size)
            else:
                raise ValueError(f'Unsupported site input: {path}')

        config = self.source / 'config.toml'
        _reject_link(config)
        if not config.is_file():
            raise ValueError(f'Site config is missing: {config}')
        visit(config)
        for name in INPUT_DIRECTORIES:
            directory = self.source / name
            _reject_link(directory)
            if directory.exists():
                if not directory.is_dir():
                    raise ValueError(f'Site input must be a directory: {directory}')
                visit(directory)
        return files, directories

    def _destination(self, relative):
        destination = self.target / relative
        _reject_link_parents(destination)
        # Resolve immediately before any write/delete, never trusting a path
        # merely because it was safe during the previous sync.
        if self.target not in destination.resolve().parents:
            raise ValueError(f'Staging path escaped its target: {destination}')
        return destination

    def sync(self):
        files, directories = self._scan()
        _reject_link_parents(self.target)
        if not self.target.is_dir():
            raise ValueError(f'Staging target is missing: {self.target}')
        original_data = None
        if ORIGINAL_BODIES in files:
            # Only the private staging data contains original Markdown. Templates
            # use it to preserve Zola's reading-time semantics before math markup.
            bodies = {}
            body_cache = {}
            for relative, fingerprint in files.items():
                if relative.parts[0] != 'content' or relative.suffix.lower() != '.md':
                    continue
                cached = self._body_cache.get(relative)
                if cached is not None and cached[0] == fingerprint:
                    body = cached[1]
                else:
                    body = _markdown_body((self.source / relative).read_bytes().decode('utf-8'))
                body_cache[relative] = (fingerprint, body)
                bodies[relative.relative_to('content').as_posix()] = body
            self._body_cache = body_cache
            original_data = json.dumps(bodies, ensure_ascii=False, sort_keys=True).encode('utf-8')
            files[ORIGINAL_BODIES] = hashlib.sha256(original_data).digest()
        changed = []
        # Deletions first also support changing a file into a directory.
        for relative in sorted(self._snapshot.keys() - files.keys()):
            destination = self._destination(relative)
            if destination.exists():
                if not destination.is_file():
                    raise ValueError(f'Unexpected staging directory: {destination}')
                destination.unlink()
            del self._snapshot[relative]
            changed.append(relative)

        # Only remove formerly owned, empty directories. Zola may have written
        # other files into the staging tree, which this helper never owns.
        for relative in sorted(self._directories - directories,
                               key=lambda item: len(item.parts), reverse=True):
            destination = self._destination(relative)
            if destination.exists():
                if not destination.is_dir():
                    raise ValueError(f'Unexpected staging file: {destination}')
                if not any(destination.iterdir()):
                    destination.rmdir()
                    self._directories.discard(relative)

        for relative in sorted(directories, key=lambda item: len(item.parts)):
            self._destination(relative).mkdir(parents=True, exist_ok=True)
            self._directories.add(relative)

        # Publish reading data before content so a live rebuild sees its matching
        # original body as soon as the transformed Markdown changes.
        for relative, fingerprint in sorted(files.items(),
                                            key=lambda item: (item[0] != ORIGINAL_BODIES, item[0])):
            destination = self._destination(relative)
            if self._snapshot.get(relative) == fingerprint and destination.is_file():
                continue
            if destination.exists() and relative not in self._snapshot:
                raise ValueError(f'Refusing to replace an unowned staging file: {destination}')
            data = original_data if relative == ORIGINAL_BODIES else (self.source / relative).read_bytes()
            if relative.parts[0] == 'content' and relative.suffix.lower() == '.md':
                data = protect_math(data.decode('utf-8')).encode('utf-8')
            # Zola 0.23.4 ignores paired Linux rename events. Stage writes in an
            # unwatched child of its non-recursive root watch, so publishing
            # produces a supported RenameMode::To while remaining atomic.
            write_directory = self._destination(Path('.prepare-tmp'))
            write_directory.mkdir(exist_ok=True)
            temporary = None
            try:
                with tempfile.NamedTemporaryFile(dir=write_directory,
                                                 prefix='.prepare-', delete=False) as output:
                    temporary = Path(output.name)
                    output.write(data)
                os.replace(temporary, destination)
            finally:
                if temporary is not None and temporary.exists():
                    temporary.unlink()
            # Keep successful work owned even if a later input fails to
            # transform, so a preview can recover on its next sync.
            self._snapshot[relative] = fingerprint
            changed.append(relative)

        self._snapshot = files
        self._directories = directories
        return sorted(changed)
