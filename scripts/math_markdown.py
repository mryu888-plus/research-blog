"""Protect TeX before Zola parses Markdown or expands template shortcodes.

Only the prepared build copy is transformed. Numeric entities keep every TeX
character (including table pipes and template braces) out of Markdown's grammar.
"""
import re


def _entities(value):
    return ''.join(f'&#{ord(character)};' for character in value)


def _placeholder(tex, display, opening, closing):
    fallback = _entities(opening + tex + closing)
    return (f'<span class="math-source" data-math-tex="{_entities(tex)}" '
            f'data-math-display="{str(display).lower()}">{fallback}</span>')


def _escaped(text, index):
    preceding = index - 1
    while preceding >= 0 and text[preceding] == '\\':
        preceding -= 1
    return (index - preceding - 1) % 2 == 1


def _continue_containers(line, containers):
    """Consume existing list/quote prefixes in their original nesting order."""
    matched = []
    for kind, width in containers:
        if kind == 'quote':
            prefix = re.match(r' {0,3}> ?', line)
            if not prefix:
                break
            line = line[prefix.end():]
        elif not line.strip():
            # A blank line may remain in a list, but an omitted quote marker
            # still ends a quoted code block nested inside that list.
            pass
        elif line.startswith(' ' * width):
            line = line[width:]
        else:
            break
        matched.append((kind, width))
    return line, tuple(matched)


def _open_containers(line, containers):
    """Recognize additional quote/list prefixes, only outside fenced code."""
    opened = list(containers)
    while True:
        quote = re.match(r' {0,3}> ?', line)
        if quote:
            opened.append(('quote', 0))
            line = line[quote.end():]
            continue
        marker = re.match(r' {0,3}(?:[-+*]|\d{1,9}[.)])( +)', line)
        if marker:
            # Five or more spaces after a marker mean one padding space plus
            # an indented code block, rather than arbitrarily deep padding.
            padding = len(marker.group(1))
            width = marker.start(1) + (padding if padding <= 4 else 1)
            opened.append(('list', width))
            line = line[width:]
            continue
        break
    return line, tuple(opened)


def _unquote_math(tex, depth):
    if not depth:
        return tex
    lines = tex.splitlines(keepends=True)
    for number in range(1, len(lines)):
        line = lines[number]
        for _ in range(depth):
            # A quote inside an ordered or nested list can be preceded by
            # more than three indentation spaces in the original source.
            match = re.match(r'[ \t]*>[ \t]?', line)
            if not match:
                break
            line = line[match.end():]
        lines[number] = line
    return ''.join(lines)


def _math_end(text, start, opening, closing, display):
    begin = start + len(opening)
    if not display and (begin == len(text) or text[begin].isspace()):
        return None
    candidate = begin
    while (candidate := text.find(closing, candidate)) != -1:
        body = text[begin:candidate]
        if not display and ('\n' in body or '\r' in body):
            return None
        if not _escaped(text, candidate):
            after = candidate + len(closing)
            if closing == '$':
                # Single dollars neither consume display delimiters nor turn
                # prices like "$5 and $10" into a formula.
                if (candidate > begin and not text[candidate - 1].isspace()
                        and text[candidate - 1] != '$'
                        and (after == len(text) or
                             (text[after] != '$' and not text[after].isdigit()))):
                    return candidate
            elif closing != '$$' or (text[candidate - 1:candidate] != '$'
                                     and text[after:after + 1] != '$'):
                return candidate
        candidate += len(closing)
    return None


def _html_end(text, start):
    """Skip tag attributes, comments, autolinks, and verbatim HTML elements."""
    if text.startswith('<!--', start):
        end = text.find('-->', start + 4)
        return len(text) if end < 0 else end + 3
    if text.startswith('<![CDATA[', start):
        end = text.find(']]>', start + 9)
        return len(text) if end < 0 else end + 3
    tag = re.match(r'</?([a-zA-Z][a-zA-Z0-9:-]*)(?=[\s/>])', text[start:])
    if not tag:
        autolink = re.match(r'<(?:[a-zA-Z][a-zA-Z0-9+.-]*:[^<>\s]*|[^<>\s]+@[^<>\s]+)>', text[start:])
        return start + autolink.end() if autolink else None
    quote = None
    end = start + tag.end()
    while end < len(text):
        character = text[end]
        if quote:
            if character == quote:
                quote = None
        elif character in ('"', "'"):
            quote = character
        elif character == '>':
            end += 1
            break
        end += 1
    if (not text.startswith('</', start)
            and tag.group(1).lower() in {'pre', 'code', 'script', 'style', 'textarea', 'math', 'svg'}):
        closing = re.search(r'</' + re.escape(tag.group(1)) + r'\s*>', text[end:], re.I)
        return end + closing.end() if closing else len(text)
    return end


def _destination_end(text, start):
    depth = 1
    cursor = start + 1
    while cursor < len(text):
        if text[cursor] == '\\':
            cursor += 2
            continue
        if text[cursor] == '(':
            depth += 1
        elif text[cursor] == ')':
            depth -= 1
            if not depth:
                return cursor + 1
        cursor += 1
    return start + 1


def protect_math(text):
    """Protect dollar and LaTeX bracket math, leaving code and metadata alone."""
    result = []
    cursor = 0
    frontmatter = re.match(r'\A\ufeff?(\+\+\+|---)[ \t]*\r?\n', text)
    if frontmatter:
        ending = re.search(r'^' + re.escape(frontmatter.group(1)) + r'[ \t]*(?:\r?\n|$)',
                           text[frontmatter.end():], re.M)
        if ending is None:
            return text
        cursor = frontmatter.end() + ending.end()
        result.append(text[:cursor])

    fence = None
    code_indent = False
    previous_blank = True
    containers = ()
    quote_depth = 0
    while cursor < len(text):
        if cursor == 0 or text[cursor - 1] == '\n':
            line_end = text.find('\n', cursor)
            line_end = len(text) if line_end < 0 else line_end + 1
            line = text[cursor:line_end]
            logical, continued = _continue_containers(line.expandtabs(4), containers)
            if fence and continued != fence[2]:
                # CommonMark closes a fenced block when its containing quote
                # or list item ends, even without an explicit closing fence.
                fence = None
                previous_blank = True
            containers = continued
            opened_container = False
            if not fence:
                logical, opened = _open_containers(logical, containers)
                opened_container = opened != containers
                containers = opened
            quote_depth = sum(kind == 'quote' for kind, _ in containers)
            relative_indent = len(logical) - len(logical.lstrip(' '))
            blank = not logical.strip()
            possible_fence = re.match(r' {0,3}(`{3,}|~{3,})([^\r\n]*)', logical)
            if fence:
                if (possible_fence and possible_fence.group(1)[0] == fence[0]
                        and len(possible_fence.group(1)) >= fence[1]
                        and not possible_fence.group(2).strip()):
                    fence = None
                result.append(line)
                cursor = line_end
                previous_blank = blank or fence is None
                continue
            if possible_fence and not (possible_fence.group(1)[0] == '`'
                                       and '`' in possible_fence.group(2)):
                fence = (possible_fence.group(1)[0], len(possible_fence.group(1)), containers)
                code_indent = False
                result.append(line)
                cursor = line_end
                previous_blank = blank
                continue
            if (code_indent and (blank or relative_indent >= 4)
                    or (previous_blank or opened_container) and relative_indent >= 4):
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

        if text[cursor] == '`':
            run = re.match(r'`+', text[cursor:]).group()
            closing = re.search(r'(?<!`)' + run + r'(?!`)', text[cursor + len(run):])
            end = cursor + len(run) + closing.end() if closing else cursor + len(run)
            result.append(text[cursor:end])
            cursor = end
            continue
        if text[cursor] == '<':
            end = _html_end(text, cursor)
            if end is not None:
                result.append(text[cursor:end])
                cursor = end
                continue
        if text.startswith(('{{', '{%'), cursor):
            close = '}}' if text.startswith('{{', cursor) else '%}'
            end = text.find(close, cursor + 2)
            if end >= 0:
                end += 2
                result.append(text[cursor:end])
                cursor = end
                continue
        if text.startswith('![', cursor):
            end = re.search(r'(?<!\\)\]', text[cursor + 2:])
            if end:
                end = cursor + 2 + end.end()
                result.append(text[cursor:end])
                cursor = end
                continue
        if text[cursor] == '(' and cursor > 0 and text[cursor - 1] == ']':
            end = _destination_end(text, cursor)
            result.append(text[cursor:end])
            cursor = end
            continue

        opening = closing = None
        display = False
        if not _escaped(text, cursor):
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
            end = _math_end(text, cursor, opening, closing, display)
            if end is not None:
                tex = _unquote_math(text[cursor + len(opening):end], quote_depth)
                result.append(_placeholder(tex, display, opening, closing))
                cursor = end + len(closing)
                continue
        result.append(text[cursor])
        cursor += 1
    return ''.join(result)
