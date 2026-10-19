"""Offline citations for the prepared Zola tree, with a public-only bibliography.

The authoring tree remains untouched. Zotero exports the small, explicit CSL
subset below; neither attachments nor annotations belong in that snapshot.
Article-local refs.bib is also accepted for existing manuscripts. This is a
strict parser of standard BibTeX entry/value syntax, not a TeX interpreter.
"""
from dataclasses import dataclass
import hashlib
import html
import json
from pathlib import Path
import re
import tomllib
import unicodedata
from urllib.parse import quote, urlsplit

from math_markdown import (_continue_containers, _destination_end, _escaped,
                           _html_end, _math_end, _open_containers)

SNAPSHOT = Path('data/references.json')
INDEX_PAGE = Path('content/references.md')
ITEM_FIELDS = {'id', 'citekey', 'title', 'author', 'issued', 'type',
               'container-title', 'DOI', 'URL', 'volume', 'issue', 'page', 'publisher'}
KEY = r'[^\s@;,\[\]{}<>"\\]+'
CITATION = re.compile(r'\[\s*@(' + KEY + r')([^\]\r\n]*)\]')
PART = re.compile(r'\s*@(' + KEY + r')\s*(?:,\s*(.*))?\s*\Z')


def _escaped_html(value):
    # Also prevent shortcode expansion and later TeX transformation in generated
    # Markdown HTML. html.escape alone does not protect braces or dollars.
    return (html.escape(str(value), quote=True).replace('{', '&#123;')
            .replace('}', '&#125;').replace('$', '&#36;'))


def _anchor(key):
    return 'ref-' + hashlib.sha256(key.encode('utf-8')).hexdigest()[:20]


def _string(value, context):
    if not isinstance(value, str) or any(ord(c) < 32 and c not in '\n\r\t' for c in value):
        raise ValueError(f'{context} must be text without control characters')
    return value


def validate_snapshot(value):
    """Reject rather than silently discard private/unknown snapshot fields."""
    if not isinstance(value, dict) or set(value) != {'version', 'items'}:
        raise ValueError('references.json requires only version and items; private fields are forbidden')
    if type(value['version']) is not int or value['version'] != 1 or not isinstance(value['items'], list):
        raise ValueError('references.json must have version 1 and an items list')
    result = {}
    for number, item in enumerate(value['items']):
        context = f'references.json item {number + 1}'
        if not isinstance(item, dict) or set(item) - ITEM_FIELDS:
            unknown = sorted(set(item) - ITEM_FIELDS) if isinstance(item, dict) else ['non-object']
            raise ValueError(f'{context}: forbidden fields: {", ".join(unknown)}')
        for required in ('id', 'citekey', 'title'):
            if not _string(item.get(required), f'{context}.{required}').strip():
                raise ValueError(f'{context}.{required} cannot be empty')
        key = item['citekey']
        if not re.fullmatch(KEY, key):
            raise ValueError(f'{context}: invalid citekey {key!r}')
        if key in result:
            raise ValueError(f'references.json: duplicate citekey {key!r}')
        for name in ITEM_FIELDS - {'author', 'issued'}:
            if name in item:
                _string(item[name], f'{context}.{name}')
        if 'author' in item:
            if not isinstance(item['author'], list):
                raise ValueError(f'{context}.author must be a list')
            for author in item['author']:
                if (not isinstance(author, dict) or not author
                        or set(author) - {'family', 'given', 'literal'}
                        or ('literal' in author and len(author) != 1)):
                    raise ValueError(f'{context}.author: unsupported or private name fields')
                for name, text in author.items():
                    _string(text, f'{context}.author.{name}')
        if 'issued' in item:
            issued = item['issued']
            if (not isinstance(issued, dict) or set(issued) != {'date-parts'}
                    or not isinstance(issued['date-parts'], list) or not issued['date-parts']):
                raise ValueError(f'{context}.issued requires date-parts only')
            for date in issued['date-parts']:
                if (not isinstance(date, list) or not 1 <= len(date) <= 3
                        or any(type(part) is not int for part in date)):
                    raise ValueError(f'{context}.issued: invalid date-parts')
        result[key] = dict(item)
    return result


def _public_url(value):
    if not isinstance(value, str) or any(c.isspace() or ord(c) < 32 for c in value):
        return None
    try:
        parsed = urlsplit(value)
        if parsed.scheme.lower() not in {'http', 'https'} or not parsed.hostname or parsed.username or parsed.password:
            return None
        # DOI/URL fields containing localhost or local file paths are never linked.
        if parsed.hostname.lower() in {'localhost', '127.0.0.1', '::1', '0.0.0.0'}:
            return None
        return value
    except ValueError:
        return None


def _doi_url(value):
    value = re.sub(r'^(?:doi:\s*|https?://(?:dx\.)?doi\.org/)', '', value, flags=re.I).strip()
    if not re.fullmatch(r'10\.\d{4,9}/\S+', value):
        return None
    return 'https://doi.org/' + quote(value, safe='/():;._-')


def format_item(item):
    """Render a readable numeric bibliography using only allowlisted fields."""
    pieces = []
    authors = [author.get('literal') or ' '.join(filter(None, [author.get('given'), author.get('family')]))
               for author in item.get('author', [])]
    if authors:
        pieces.append(_escaped_html('; '.join(authors)) + '.')
    issued = item.get('issued', {}).get('date-parts', [])
    if issued:
        pieces.append(_escaped_html(issued[0][0]) + '.')
    title = '<span class="reference-title">' + _escaped_html(item['title']) + '</span>.'
    pieces.append(title)
    if item.get('container-title'):
        pieces.append('<em>' + _escaped_html(item['container-title']) + '</em>.')
    if item.get('volume'):
        volume = _escaped_html(item['volume'])
        if item.get('issue'):
            volume += '(' + _escaped_html(item['issue']) + ')'
        pieces.append(volume + '.')
    elif item.get('issue'):
        pieces.append(_escaped_html(item['issue']) + '.')
    if item.get('page'):
        pieces.append(_escaped_html(item['page']) + '.')
    if item.get('publisher'):
        pieces.append(_escaped_html(item['publisher']) + '.')
    links = []
    if item.get('DOI') and (url := _doi_url(item['DOI'])):
        links.append(f'<a href="{_escaped_html(url)}" rel="noopener noreferrer">DOI</a>')
    if item.get('URL') and (url := _public_url(item['URL'])):
        links.append(f'<a href="{_escaped_html(url)}" rel="noopener noreferrer">原文</a>')
    if links:
        pieces.append('<span class="reference-links">' + ' · '.join(links) + '</span>')
    return ' '.join(pieces)


def _frontmatter(text):
    opening = re.match(r'\A\ufeff?(\+\+\+|---)[ \t]*\r?\n', text)
    if not opening:
        return {}, 0
    ending = re.search(r'^' + re.escape(opening.group(1)) + r'[ \t]*(?:\r?\n|$)',
                       text[opening.end():], re.M)
    if ending is None:
        raise ValueError('Unclosed Markdown frontmatter')
    body_end = opening.end() + ending.end()
    raw = text[opening.end():opening.end() + ending.start()]
    if opening.group(1) == '+++':
        return tomllib.loads(raw), body_end
    # Only routing/publication metadata is needed from YAML. Flat YAML scalars
    # cover existing authoring conventions without adding a runtime dependency.
    metadata = {}
    for line in raw.splitlines():
        match = re.match(r'^(title|slug|path|draft|render):\s*(.*?)\s*$', line)
        if match:
            key, value = match.groups()
            if value.lower() in {'true', 'false'}:
                metadata[key] = value.lower() == 'true'
            elif value.startswith('"'):
                metadata[key] = json.loads(value)
            elif value.startswith("'") and value.endswith("'"):
                metadata[key] = value[1:-1].replace("''", "'")
            else:
                metadata[key] = value.split(' #', 1)[0]
    return metadata, body_end


def transform_citations(text, items, source='article'):
    """Replace visible citation groups, preserving code, math and metadata."""
    _, cursor = _frontmatter(text)
    result = [text[:cursor]]
    numbers = {}
    mentions = {}
    fence = None
    code_indent = False
    previous_blank = True
    containers = ()
    while cursor < len(text):
        if cursor == 0 or text[cursor - 1] == '\n':
            line_end = text.find('\n', cursor)
            line_end = len(text) if line_end < 0 else line_end + 1
            line = text[cursor:line_end]
            logical, continued = _continue_containers(line.expandtabs(4), containers)
            if fence and continued != fence[2]:
                fence = None
                previous_blank = True
            containers = continued
            opened_container = False
            if not fence:
                logical, opened = _open_containers(logical, containers)
                opened_container = opened != containers
                containers = opened
            indent = len(logical) - len(logical.lstrip(' '))
            blank = not logical.strip()
            possible_fence = re.match(r' {0,3}(`{3,}|~{3,})([^\r\n]*)', logical)
            if fence:
                if (possible_fence and possible_fence.group(1)[0] == fence[0]
                        and len(possible_fence.group(1)) >= fence[1] and not possible_fence.group(2).strip()):
                    fence = None
                result.append(line)
                cursor = line_end
                previous_blank = blank or fence is None
                continue
            if possible_fence and not (possible_fence.group(1)[0] == '`' and '`' in possible_fence.group(2)):
                fence = (possible_fence.group(1)[0], len(possible_fence.group(1)), containers)
                code_indent = False
                result.append(line)
                cursor = line_end
                previous_blank = blank
                continue
            if (code_indent and (blank or indent >= 4)
                    or (previous_blank or opened_container) and indent >= 4):
                code_indent = True
                result.append(line)
                cursor = line_end
                previous_blank = blank
                continue
            code_indent = False
            previous_blank = blank
            if re.match(r' {0,3}\[[^\]\r\n]+\]:', logical):
                result.append(line)
                cursor = line_end
                continue
        end = None
        if text[cursor] == '`':
            run = re.match(r'`+', text[cursor:]).group()
            close = re.search(r'(?<!`)' + run + r'(?!`)', text[cursor + len(run):])
            end = cursor + len(run) + close.end() if close else cursor + len(run)
        elif text[cursor] == '<':
            end = _html_end(text, cursor)
        elif text.startswith(('{{', '{%'), cursor):
            close = '}}' if text.startswith('{{', cursor) else '%}'
            found = text.find(close, cursor + 2)
            if found >= 0:
                end = found + 2
        elif text.startswith('![', cursor):
            closing = re.search(r'(?<!\\)\]', text[cursor + 2:])
            if closing:
                end = cursor + 2 + closing.end()
        elif text[cursor] == '(' and cursor > 0 and text[cursor - 1] == ']':
            end = _destination_end(text, cursor)
        if end is not None:
            result.append(text[cursor:end])
            cursor = end
            continue
        if not _escaped(text, cursor):
            opening = closing = None
            display = False
            if text.startswith('$$', cursor) and text[cursor + 2:cursor + 3] != '$':
                opening = closing = '$$'
                display = True
            elif text[cursor] == '$' and text[cursor - 1:cursor] != '$':
                opening = closing = '$'
            elif text.startswith(r'\(', cursor):
                opening, closing = r'\(', r'\)'
            elif text.startswith(r'\[', cursor):
                opening, closing = r'\[', r'\]'
                display = True
            if opening:
                found = _math_end(text, cursor, opening, closing, display)
                if found is not None:
                    end = found + len(closing)
                    result.append(text[cursor:end])
                    cursor = end
                    continue
            citation = CITATION.match(text, cursor)
            if citation and text[citation.end():citation.end() + 1] not in {'(', '['}:
                parts = [PART.fullmatch(part) for part in text[cursor + 1:citation.end() - 1].split(';')]
                if all(parts):
                    links = []
                    for part in parts:
                        key, locator = part.groups()
                        if key not in items:
                            line = text.count('\n', 0, cursor) + 1
                            raise ValueError(f'{source}:{line}: missing reference @{key}; export it to data/references.json or add refs.bib')
                        number = numbers.setdefault(key, len(numbers) + 1)
                        occurrence = len(mentions.setdefault(key, [])) + 1
                        cite_id = f'cite-{_anchor(key)}-{occurrence}'
                        mentions[key].append(cite_id)
                        label = str(number) + (', ' + locator.strip() if locator else '')
                        links.append(f'<a id="{cite_id}" href="#{_anchor(key)}" aria-label="参考文献 {number}">{_escaped_html(label)}</a>')
                    result.append('<span class="citation">[' + '; '.join(links) + ']</span>')
                    cursor = citation.end()
                    continue
        result.append(text[cursor])
        cursor += 1
    if numbers:
        rows = []
        for key, number in numbers.items():
            backlinks = ' '.join(f'<a href="#{identifier}" aria-label="返回正文引用 {number}，第 {index + 1} 处">↩{index + 1 if len(mentions[key]) > 1 else ""}</a>'
                                 for index, identifier in enumerate(mentions[key]))
            rows.append(f'<li id="{_anchor(key)}">{format_item(items[key])} <span class="reference-backlinks">{backlinks}</span></li>')
        result.append('\n\n<section class="article-references" aria-labelledby="reference-heading">\n<h2 id="reference-heading">参考文献</h2>\n<ol>\n'
                      + '\n'.join(rows) + '\n</ol>\n</section>\n')
    return ''.join(result), list(numbers)


def _tex_text(value):
    """Decode common BibTeX text accents; fail on unhandled TeX commands."""
    accents = {"'": '\u0301', '`': '\u0300', '"': '\u0308', '^': '\u0302', '~': '\u0303',
               '=': '\u0304', '.': '\u0307', 'v': '\u030c', 'u': '\u0306', 'H': '\u030b', 'c': '\u0327'}
    value = re.sub(r'\\([\'`"^~=\.]|[vuHc](?=[{\s]))\s*\{?([A-Za-z])\}?',
                   lambda m: unicodedata.normalize('NFC', m[2] + accents[m[1]]), value)
    substitutions = {'\\&': '&', '\\%': '%', '\\_': '_', '\\#': '#', '\\$': '$', '\\{': '{', '\\}': '}',
                     '\\LaTeX': 'LaTeX', '\\TeX': 'TeX', '\\ss': 'ß', '\\ae': 'æ', '\\AE': 'Æ',
                     '\\oe': 'œ', '\\OE': 'Œ', '\\o': 'ø', '\\O': 'Ø', '\\l': 'ł', '\\L': 'Ł'}
    value = re.sub(r'\\(?:[A-Za-z]+|.)', lambda m: substitutions.get(m[0], m[0]), value)
    if re.search(r'\\[A-Za-z]+', value):
        raise ValueError(f'Unsupported TeX command in BibTeX public text: {value!r}; export CSL JSON instead')
    return re.sub(r'[{}]', '', value).replace('~', ' ').strip()


class _BibParser:
    def __init__(self, text, source):
        self.text, self.source, self.cursor = text, source, 0
        self.strings = {name: name.capitalize() for name in ('jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec')}

    def error(self, message):
        raise ValueError(f'{self.source}:{self.text.count(chr(10), 0, self.cursor) + 1}: {message}')

    def whitespace(self):
        while self.cursor < len(self.text):
            if self.text[self.cursor].isspace():
                self.cursor += 1
            elif self.text[self.cursor] == '%':
                end = self.text.find('\n', self.cursor)
                self.cursor = len(self.text) if end < 0 else end + 1
            else:
                break

    def word(self):
        self.whitespace()
        match = re.match(r'[^\s=,{}()#"@]+', self.text[self.cursor:])
        if not match:
            self.error('Expected a BibTeX identifier')
        self.cursor += match.end()
        return match[0]

    def expect(self, token):
        self.whitespace()
        if not self.text.startswith(token, self.cursor):
            self.error(f'Expected {token!r}')
        self.cursor += len(token)

    def enclosed(self, opening):
        self.cursor += 1
        start = self.cursor
        depth = 0
        while self.cursor < len(self.text):
            character = self.text[self.cursor]
            if character == '\\':
                self.cursor += 2
                continue
            if character == '{':
                depth += 1
            elif character == '}':
                if opening == '{' and depth == 0:
                    value = self.text[start:self.cursor]
                    self.cursor += 1
                    return value
                depth -= 1
            elif opening == '"' and character == '"' and depth == 0:
                value = self.text[start:self.cursor]
                self.cursor += 1
                return value
            self.cursor += 1
        self.error('Unclosed BibTeX value')

    def value(self):
        values = []
        while True:
            self.whitespace()
            if self.cursor >= len(self.text):
                self.error('Missing BibTeX field value')
            if self.text[self.cursor] in '{"':
                values.append(self.enclosed(self.text[self.cursor]))
            else:
                word = self.word()
                if word.isdigit():
                    values.append(word)
                elif word.lower() in self.strings:
                    values.append(self.strings[word.lower()])
                else:
                    self.error(f'Unknown BibTeX string {word!r}')
            self.whitespace()
            if self.text[self.cursor:self.cursor + 1] != '#':
                return ''.join(values)
            self.cursor += 1

    def parse(self):
        entries = {}
        while True:
            self.whitespace()
            if self.cursor >= len(self.text):
                break
            # Text outside entries is a standard BibTeX comment.
            found = self.text.find('@', self.cursor)
            if found < 0:
                break
            self.cursor = found + 1
            kind = self.word().lower()
            self.whitespace()
            opening = self.text[self.cursor:self.cursor + 1]
            if opening not in {'{', '('}:
                self.error('Expected an entry delimiter')
            close = '}' if opening == '{' else ')'
            if kind == 'comment':
                if opening == '{':
                    self.enclosed('{')
                else:
                    end = self.text.find(')', self.cursor + 1)
                    if end < 0:
                        self.error('Unclosed comment')
                    self.cursor = end + 1
                continue
            self.cursor += 1
            if kind == 'preamble':
                self.value()
                self.expect(close)
                continue
            key = self.word()
            if kind == 'string':
                self.expect('=')
                self.strings[key.lower()] = self.value()
                self.whitespace()
                if self.text[self.cursor:self.cursor + 1] == ',':
                    self.cursor += 1
                self.expect(close)
                continue
            self.expect(',')
            fields = {}
            while True:
                self.whitespace()
                if self.text[self.cursor:self.cursor + 1] == close:
                    self.cursor += 1
                    break
                field = self.word().lower()
                self.expect('=')
                if field in fields:
                    self.error(f'Duplicate BibTeX field {field!r}')
                fields[field] = self.value()
                self.whitespace()
                if self.text[self.cursor:self.cursor + 1] == close:
                    self.cursor += 1
                    break
                self.expect(',')
            if key in entries:
                self.error(f'Duplicate BibTeX key {key!r}')
            entries[key] = (kind, fields)
        return entries


def _authors(value):
    # Split "and"/commas outside braces so organizations remain literal names.
    names, start, depth = [], 0, 0
    for match in re.finditer(r'[{}]|\s+and\s+', value, re.I):
        if match[0] == '{':
            depth += 1
        elif match[0] == '}':
            depth -= 1
        elif depth == 0:
            names.append(value[start:match.start()].strip())
            start = match.end()
    names.append(value[start:].strip())
    result = []
    for name in filter(None, names):
        if name.startswith('{') and name.endswith('}'):
            result.append({'literal': _tex_text(name)})
        elif ',' in name:
            parts = [_tex_text(part.strip()) for part in name.split(',')]
            if len(parts) == 2:
                result.append({'family': parts[0], 'given': parts[1]})
            elif len(parts) == 3:
                result.append({'family': parts[0] + ', ' + parts[1], 'given': parts[2]})
            else:
                raise ValueError(f'Unsupported BibTeX author name: {name!r}')
        else:
            parts = _tex_text(name).split()
            split = len(parts) - 1
            for index, part in enumerate(parts[:-1]):
                if part[0].islower():
                    split = index
                    break
            result.append({'family': ' '.join(parts[split:]), 'given': ' '.join(parts[:split])})
    return result


def read_bibtex(text, source='refs.bib'):
    entries = _BibParser(text, source).parse()
    resolved = {}

    def resolve(key, chain=()):
        if key in resolved:
            return resolved[key]
        if key in chain or key not in entries:
            raise ValueError(f'{source}: missing or cyclic crossref {key!r}')
        _, fields = entries[key]
        if fields.get('crossref'):
            fields = {**resolve(fields['crossref'], (*chain, key)), **fields}
        resolved[key] = fields
        return fields

    result = {}
    for key, (kind, _) in entries.items():
        fields = resolve(key)
        if not fields.get('title'):
            raise ValueError(f'{source}: @{key} is missing title')
        item = {'id': key, 'citekey': key, 'title': _tex_text(fields['title']), 'type': kind}
        if fields.get('author'):
            item['author'] = _authors(fields['author'])
        if fields.get('year'):
            if not re.fullmatch(r'\d{1,4}', fields['year'].strip()):
                raise ValueError(f'{source}: @{key} year must be numeric; export CSL JSON instead')
            item['issued'] = {'date-parts': [[int(fields['year'])]]}
        mapping = {'journal': 'container-title', 'booktitle': 'container-title', 'doi': 'DOI',
                   'url': 'URL', 'volume': 'volume', 'number': 'issue', 'pages': 'page', 'publisher': 'publisher'}
        for original, target in mapping.items():
            if fields.get(original):
                item[target] = fields[original].strip() if target in {'DOI', 'URL'} else _tex_text(fields[original])
        result[key] = item
    return validate_snapshot({'version': 1, 'items': list(result.values())})


def _article_link(article, key):
    # Zola owns routing, including Unicode transliteration, slug/path overrides,
    # parent sections and deployment base URLs. Never guess its slug algorithm.
    arguments = {'path': article['source'], 'anchor': _anchor(key),
                 'label': _escaped_html(article['title'])}
    return '{{ <reference_link ' + ' '.join(name + '=' + json.dumps(value, ensure_ascii=False)
                                            for name, value in arguments.items()) + ' /> }}'


@dataclass
class ReferencePreparation:
    overrides: dict
    excluded: set


def prepare_references(source, files):
    """Return generated bytes and excluded raw bibliographies for staging."""
    source = Path(source)
    bibliographies = {path for path in files if path.parts[0] == 'content' and path.suffix.lower() == '.bib'}
    texts = {path: (source / path).read_bytes().decode('utf-8') for path in files
             if path.parts[0] == 'content' and path.suffix.lower() == '.md'}
    metadata = {path: _frontmatter(text)[0] for path, text in texts.items()}
    snapshot = {}
    if SNAPSHOT in files:
        snapshot = validate_snapshot(json.loads((source / SNAPSHOT).read_text(encoding='utf-8-sig')))
    parsed_bibs = {path: read_bibtex((source / path).read_text(encoding='utf-8-sig'), str(path))
                   for path in bibliographies if path.name.lower() == 'refs.bib'}
    overrides = {}
    published = {}
    backlinks = {}
    active = SNAPSHOT in files or bool(bibliographies)
    for relative, text in texts.items():
        if relative == INDEX_PAGE and metadata[relative].get('extra', {}).get('reference_index'):
            continue
        local = parsed_bibs.get(relative.parent / 'refs.bib', {})
        items = {**local, **snapshot}
        transformed, keys = transform_citations(text, items, str(relative))
        if not keys:
            continue
        active = True
        overrides[relative] = transformed.encode('utf-8')
        meta = metadata[relative]
        ancestors = [path for path in metadata if path.name == '_index.md' and path.parent in relative.parents]
        if (meta.get('draft') or meta.get('render') is False
                or any(metadata[path].get('draft') or metadata[path].get('render') is False for path in ancestors)):
            continue
        article = {'title': str(meta.get('title', relative.stem)),
                   'source': '@/' + relative.relative_to('content').as_posix()}
        for key in keys:
            # Article-local keys can refer to different papers. Do not merge an
            # ambiguous key silently on the global bibliography page.
            if key in published and published[key] != items[key]:
                raise ValueError(f'{relative}: conflicting public reference @{key}; use distinct citekeys')
            published[key] = items[key]
            backlinks.setdefault(key, []).append(article)
    if active:
        if INDEX_PAGE in files and not metadata[INDEX_PAGE].get('extra', {}).get('reference_index'):
            raise ValueError('content/references.md conflicts with the generated bibliography; move it or mark extra.reference_index = true')
        rows = []
        for key in sorted(published, key=lambda key: (published[key]['title'].casefold(), key)):
            related = ' · '.join(_article_link(article, key) for article in backlinks[key])
            rows.append(f'<li id="{_anchor(key)}">{format_item(published[key])}<div class="reference-articles">引用文章：{related}</div></li>')
        content = ('+++\ntitle = "文献"\ndescription = "已发表文章引用的文献与相关笔记。"\npath = "references"\ntemplate = "page.html"\n[extra]\nreference_index = true\n+++\n\n')
        if rows:
            content += '<ol class="reference-library">\n' + '\n'.join(rows) + '\n</ol>\n'
        else:
            content += '<p class="reference-empty">已发表文章中的文献引用将出现在这里。</p>\n'
        overrides[INDEX_PAGE] = content.encode('utf-8')
        # Even a template deliberately exposing this data sees only references
        # cited by published pages, never uncited or draft-only library entries.
        overrides[SNAPSHOT] = json.dumps({'version': 1, 'items': list(published.values())},
                                        ensure_ascii=False, sort_keys=True).encode('utf-8')
    return ReferencePreparation(overrides, bibliographies)
