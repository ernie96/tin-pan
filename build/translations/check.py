"""
Merges the translation batches into TinPan/Localization/translations.json and checks them:
every English text in the code has Romanian, Ukrainian and Spanish; placeholders match; plural
forms have the right count; and access keys (the letter after "_") are unique on each page,
menu and form, in every language. Tin Pan 2 (v2/, Rust) reads the same file, so its texts and
menus are checked and kept too.

    python -I build/translations/check.py [batch.py ...]

With no batches it only checks the existing translations.json against the code.
"""
import glob
import json
import os
import re
import runpy
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
APP = os.path.join(ROOT, 'TinPan')
OUT = os.path.join(APP, 'Localization', 'translations.json')
LANGS = ['Romanian', 'Ukrainian', 'Spanish']
PLURAL_FORMS = [3, 3, 2]


def unescape(s):
    return json.loads('"' + s + '"')


def keys_in_code():
    keys = []
    single = re.compile(r'(?<![\w.])T\("((?:[^"\\]|\\.)*)"\s*[,)]')
    plural = re.compile(r'Plural\([^,]+,\s*"((?:[^"\\]|\\.)*)",\s*"((?:[^"\\]|\\.)*)"\)')
    xaml = re.compile(r"\{l:T '((?:[^'\\]|\\.)*)'\}")
    for path in glob.glob(os.path.join(APP, '**', '*.cs'), recursive=True):
        if os.sep + 'obj' + os.sep in path:
            continue
        text = open(path, encoding='utf-8').read()
        keys += [unescape(m.group(1)) for m in single.finditer(text)]
        keys += [unescape(m.group(1)) + '|' + unescape(m.group(2)) for m in plural.finditer(text)]
    for path in glob.glob(os.path.join(APP, '**', '*.xaml'), recursive=True):
        if os.sep + 'obj' + os.sep in path:
            continue
        keys += [m.group(1).replace("\\'", "'").replace('&amp;', '&').replace('&quot;', '"') for m in xaml.finditer(open(path, encoding='utf-8').read())]
    keys += ["Tin Pan, like all my apps, is dedicated to Jenni and Diana.", "Tin Pan was created by Ernesto Melendez (github.com/ernie96/tin-pan).", "Automatic (Windows language)", "Romanian", "Ukrainian", "Spanish", "English"]
    # Tin Pan 2's texts last, so Tin Pan 1's keep their order in the file.
    keys += v2_keys()
    return [k for k in dict.fromkeys(keys) if re.search('[A-Za-z]{2}', k)]


V2 = os.path.join(ROOT, 'v2', 'crates')
RUST_TEXT = r'"((?:[^"\\]|\\.)*)"'


def rust_unescape(s):
    return s.replace('\\"', '"').replace('\\n', '\n').replace('\\\\', '\\')


def v2_sources():
    for path in glob.glob(os.path.join(V2, '**', '*.rs'), recursive=True):
        text = open(path, encoding='utf-8').read()
        # Tests check English on purpose; their texts aren't the app's.
        yield path, re.split(r'#\[cfg\((?:all\()?test', text)[0]


def v2_keys():
    """Tin Pan 2's texts: t(), tf() and plural() calls, its command table, and texts its code picks before t()."""
    keys = []
    calls = re.compile(r'\b(?:t|tf|wx_label)\(\s*&?' + RUST_TEXT)
    plural = re.compile(r'\bplural\([^,]+,\s*' + RUST_TEXT + r',\s*' + RUST_TEXT)
    table = re.compile(r'\bentry\(G::\w+,\s*' + RUST_TEXT)
    picked = re.compile(r'(?:=> |submenu = Some\()' + RUST_TEXT + r'[,)]')
    # Form fields and their buttons and pickers, translated where the form shows them.
    fields = re.compile(r'(?:\bField::(?:check|heading|choice|number)\(|\blabel: |\bbutton: |\bpicker_title: |\bfiles: Some\(\()' + RUST_TEXT)
    # Texts kept in tables and translated where they're shown: ("False", "Hide songs I disliked") and the like.
    pairs = re.compile(r'\((?:"[^"]*"|[A-Z_]+),\s*' + RUST_TEXT + r'\)')
    for path, text in v2_sources():
        keys += [rust_unescape(m.group(1)) for m in calls.finditer(text)]
        keys += [rust_unescape(m.group(1)) + '|' + rust_unescape(m.group(2)) for m in plural.finditer(text)]
        keys += [rust_unescape(m.group(1)) for m in table.finditer(text)]
        if path.endswith(('commands.rs', 'filters.rs')):
            keys += [rust_unescape(m.group(1)) for m in picked.finditer(text)]
        if path.endswith(('filters.rs', 'settings.rs', 'main.rs', 'downloads.rs', 'views.rs', 'discover.rs', 'editing.rs', 'uploads.rs', 'help.rs', 'personas.rs', 'listen.rs', 'voice.rs')):
            keys += [rust_unescape(m.group(1)) for m in fields.finditer(text)]
        if path.endswith('filters.rs'):
            keys += [rust_unescape(m.group(1)) for m in pairs.finditer(text)]
        if path.endswith('listen.rs'):
            # Listen for Credits' profile choices, ("ai_hobbyist", "I make songs with AI as a hobby") and the like.
            keys += [rust_unescape(m.group(1)) for m in re.finditer(r'\("[a-z_]+",\s*' + RUST_TEXT + r'\)', text)]
        if path.endswith('create_window.rs'):
            keys += [rust_unescape(m.group(2)) for m in CREATE_FIELD.finditer(text)]
            keys += [rust_unescape(m.group(1)) for m in CREATE_PAIR.finditer(text)]
        if path.endswith(('making.rs', 'editing.rs', 'personas.rs')):
            keys += [rust_unescape(m.group(1)) for m in TOOL_FIELD.finditer(text)]
            keys += [rust_unescape(m.group(2)) for m in TOOL_SLIDER_BOX.finditer(text)]
            keys += [rust_unescape(m.group(1)) for m in WORD_PAIRS.finditer(text)]
        if path.endswith('help.rs'):
            # The full key guide: every text in full_guide(), each translated where it's shown.
            guide = text.split('pub fn full_guide', 1)[1].split(r'.join("\n\n")', 1)[0]
            keys += [rust_unescape(m.group(1)) for m in re.finditer(RUST_TEXT, guide) if m.group(1) not in ('\\n',)]
            keys += [rust_unescape(m.group(1)) for m in re.finditer(r'\bconst (?:ATTRIBUTION|DEDICATION): &str = ' + RUST_TEXT, text)]
        if path.endswith('settings_page.rs'):
            keys += [rust_unescape(m.group(1)) for r in (SETTINGS_FIELD, SETTINGS_BUTTON, SETTINGS_ACTION) for m in r.finditer(text)]
            for m in WORD_PAIRS.finditer(text):
                keys.append(rust_unescape(m.group(2) or m.group(1)))
            keys += [rust_unescape(m.group(2)) for m in CREATE_SLIDER_BOX.finditer(text)]
    # Words for the screen reader's ears only: Suno's own values and the like are left out, except the
    # single words Tin Pan 2 says after a song in a list.
    row_words = {'marked'}
    return [k for k in keys if k in row_words or (re.search('[a-z]', k) and not re.fullmatch(r'[a-z_]+', k))]


# The Create window's headings, labels and buttons: heading(&panel, &s, "...") and the like.
CREATE_FIELD = re.compile(r'\b(heading|text_box|choice|check|button|slider)\(&panel, &s, ' + RUST_TEXT)
# Its paired buttons, ("Style _boost", App::boost_style), and each slider's "Let Suno choose" box.
# The forms that stay open (making.rs): form.text("_Title", ...) and the like, and each slider's second text.
TOOL_FIELD = re.compile(r'\bform\.(?:text|time|choice|check|slider|plain_slider|ranged_slider|button)\(\s*' + RUST_TEXT)
TOOL_SLIDER_BOX = re.compile(r'\bform\.slider\(\s*' + RUST_TEXT + r',\s*' + RUST_TEXT)
# The Settings tab (settings_page.rs): b.check("...") and the like, its row buttons and its account buttons.
SETTINGS_FIELD = re.compile(r'\bb\.(?:group|label|check|choice|slider)\(\s*' + RUST_TEXT)
SETTINGS_BUTTON = re.compile(r'\bb\.button\(&\w+,\s*' + RUST_TEXT)
SETTINGS_ACTION = re.compile(r'\(' + RUST_TEXT + r',\s*Action::')
# Tables of (key, words) pairs whose words are shown: SPEECH_LEVELS, STRENGTHS, PROFILES, LANGUAGES' English names.
WORD_PAIRS = re.compile(r'\("[A-Za-z]*",\s*' + RUST_TEXT + r'(?:,\s*' + RUST_TEXT + r')?\)')
CREATE_PAIR = re.compile(r'\(' + RUST_TEXT + r', App::')
CREATE_SLIDER_BOX = re.compile(r'\bslider\(&panel, &s, ' + RUST_TEXT + r', ' + RUST_TEXT)


def v2_command_groups():
    """Tin Pan 2's menus, from its command table, the way commands.rs builds them."""
    text = open(os.path.join(V2, 'tinpan', 'src', 'commands.rs'), encoding='utf-8').read()
    groups = {}
    # A table line wraps its entry in its submenu's function (sharing(...), download(...)), in any order with after_line(...).
    submenus = {'sharing': 'S_haring', 'download': 'D_ownload', 'related': 'Related so_ngs', 'comments': '_Comments', 'make': '_Make from it'}
    for line in text.splitlines():
        m = re.search(r'\bentry\(G::(\w+),\s*' + RUST_TEXT, line)
        if not m:
            continue
        group, label = m.group(1), rust_unescape(m.group(2))
        sub = next((name for call, name in submenus.items() if call + '(' in line[:m.start()]), None)
        if sub:
            groups.setdefault(f'Tin Pan 2 menu {group}, {sub}', []).append(label)
            parent = groups.setdefault(f'Tin Pan 2 menu {group}', [])
            if sub not in parent:
                parent.append(sub)
        else:
            groups.setdefault(f'Tin Pan 2 menu {group}', []).append(label)
    swaps = {'Li_ke': 'Remove li_ke', 'D_islike': 'Remove d_islike', 'Move to tr_ash': 'Restore from tr_ash',
             'Make p_ublic': 'Make p_rivate', 'Stop remi_xes': 'Allow remi_xes',
             'Not interes_ted: fewer like this': 'Suggest songs like this a_gain'}
    for name in [g for g in groups if g.startswith('Tin Pan 2 menu Song')]:
        groups[name + ' (the other labels)'] = [swaps.get(l, l) for l in groups[name]]
    create = open(os.path.join(V2, 'tinpan', 'src', 'create_window.rs'), encoding='utf-8').read()
    groups['Tin Pan 2 Create window'] = [rust_unescape(m.group(2)) for m in CREATE_FIELD.finditer(create) if m.group(1) != 'heading']
    groups['Tin Pan 2 Create window'] += [rust_unescape(m.group(1)) for m in CREATE_PAIR.finditer(create)]
    # Each form in making.rs is one function; the Style buttons it adds are its too.
    making = open(os.path.join(V2, 'tinpan', 'src', 'making.rs'), encoding='utf-8').read()
    making += open(os.path.join(V2, 'tinpan', 'src', 'editing.rs'), encoding='utf-8').read()
    style_buttons = [rust_unescape(m.group(1)) for m in TOOL_FIELD.finditer(making.split('fn form_style_buttons', 1)[1].split('    pub(crate) fn', 1)[0])]
    for chunk in making.split('    pub(crate) fn ')[1:]:
        labels = [rust_unescape(m.group(1)) for m in TOOL_FIELD.finditer(chunk)]
        if len(labels) > 1:
            if 'form_style_buttons(' in chunk:
                labels += style_buttons
            groups['Tin Pan 2 form ' + chunk.split('(', 1)[0]] = labels
    page = open(os.path.join(V2, 'tinpan', 'src', 'settings_page.rs'), encoding='utf-8').read()
    groups['Tin Pan 2 Settings tab'] = [rust_unescape(m.group(1)) for r in (SETTINGS_FIELD, SETTINGS_BUTTON, SETTINGS_ACTION) for m in r.finditer(page) if '_' in m.group(1)]
    account = open(os.path.join(V2, 'tinpan', 'src', 'account_page.rs'), encoding='utf-8').read()
    groups['Tin Pan 2 Account tab'] = [rust_unescape(m.group(1)) for m in re.finditer(r'wx_label\(' + RUST_TEXT, account)]
    return groups


def xaml_groups():
    groups = {}
    for path in glob.glob(os.path.join(APP, '**', '*.xaml'), recursive=True):
        if os.sep + 'obj' + os.sep in path:
            continue
        text = open(path, encoding='utf-8').read()
        labels = [m.group(1).replace("\\'", "'") for m in re.finditer(r"\{l:T '((?:[^'\\]|\\.)*)'\}", text)]
        # The playlist list's Applications-key menu is its own group.
        menu = re.search(r'<ContextMenu>(.*?)</ContextMenu>', text, re.S)
        if menu:
            inside = [m.group(1) for m in re.finditer(r"\{l:T '((?:[^'\\]|\\.)*)'\}", menu.group(1))]
            groups[os.path.basename(path) + ' context menu'] = inside
            labels = [l for l in labels if l not in inside or labels.count(l) > 1]
        groups[os.path.basename(path)] = labels
    return groups


def command_groups():
    """Each menu and submenu is its own group; a submenu's own label belongs to the menu it sits in."""
    text = open(os.path.join(APP, 'ViewModels', 'MainViewModel.Commands.cs'), encoding='utf-8').read()
    headers = {m.group(1): unescape(m.group(2)) for m in re.finditer(r'"(\w+)" => T\("((?:[^"\\]|\\.)*)"\),', text)}
    groups = {}
    for m in re.finditer(r'new\("(\w+)", T\("((?:[^"\\]|\\.)*)"\)([^\n]*)', text):
        group, label, rest = m.group(1), unescape(m.group(2)), m.group(3)
        sub = re.search(r'Submenu: "(\w+)"', rest)
        if sub:
            parent = groups.setdefault('Menu ' + group, [])
            if headers.get(sub.group(1)) and headers[sub.group(1)] not in parent:
                parent.append(headers[sub.group(1)])
            groups.setdefault(f'Menu {group}, {sub.group(1)}', []).append(label)
        else:
            groups.setdefault('Menu ' + group, []).append(label)
    # Labels that swap with the song: check both forms.
    share = groups['Menu Song, Share']
    groups['Menu Song, Share (private, remixes off)'] = [{'Make p_ublic': 'Make p_rivate', 'Stop remi_xes': 'Allow remi_xes'}.get(l, l) for l in share]
    groups['Playlist songs menu'] = groups['Menu Song'] + ['Remove from playlist', 'Move up', 'Move down']
    return groups


FORMS = {
    'Extend form': ["Extend f_rom (seconds, or minutes:seconds)", "_Title", "_Lyrics for the new part", "_Style", "Style _boost", "Style i_deas...", "_Exclude styles", "_Model",
                    "_Weirdness, 0 to 100 (empty lets Suno choose)", "Style in_fluence, 0 to 100 (empty lets Suno choose)", "_Persona", "Ma_x mode"],
    'Cover form': ["_Title", "_Lyrics (change them, or keep the original words)", "_How to change the lyrics (for example: make the chorus rhyme)",
                   "_Rewrite the lyrics now", "Back to the ori_ginal words", "_Instrumental (no singing)", "New _style", "Style _boost", "Style i_deas...", "_Exclude styles",
                   "_Audio influence: how closely to follow the original, 0 to 100 (empty lets Suno choose)", "_Weirdness, 0 to 100 (empty lets Suno choose)",
                   "Style in_fluence, 0 to 100 (empty lets Suno choose)", "_Model", "_Persona", "Ma_x mode"],
    'Remaster form': ["Remaster _model", "Remaster s_trength", "Remaster _kind", "_Style (optional)", "_Freedom, 0 to 100 (empty keeps Suno's 0)",
                      "T_one, 0 to 100 (empty keeps Suno's 50)", "_Clarity, 0 to 100 (empty keeps Suno's 50)", "Stre_ngth, 0 to 100 (empty keeps Suno's 50)",
                      "Stereo _width, 0 to 100 (empty keeps Suno's 50)"],
    'Filters form': ["_Workspace", "_Disliked songs", "Songs in the _trash", "_Stems", "Suno Stud_io songs", "_Liked songs",
                     "Pu_blic or private", "_Uploaded songs", "_Covers", "_Extended songs", "_Remasters", "W_hole songs (joined parts)",
                     "Songs with a _persona", "Songs in a pla_ylist", "Downloaded be_fore", "_Order"],
    'Speed form': ["_Speed in percent: 100 is as it is, 50 is half speed, 200 is double", "_Keep the pitch (otherwise it gets higher when faster, like a tape)", "_Title"],
    'Reverse form': ["_Title"],
    'Crop form': ["_From (seconds, or minutes:seconds)", "_To (seconds, or minutes:seconds)", "_What to do with that part", "T_itle (empty says what was done)"],
    'Fade form': ["Fade _in over this many seconds (empty for none)", "Fade _out over this many seconds (empty for none)", "_Title (empty says what was done)"],
    'Cover picture form': ["_New cover", "_Description, for a picture Suno draws"],
    'Persona form': ["_Name", "_Description (the voice and style, for example)", "_Public: others can use it too"],
    'Album form': ["_Name", "_Description", "_Kind", "_Publish it now (otherwise it stays a draft on Suno)"],
    'Custom model form': ["_Playlist with the songs to learn from", "_Name, up to 16 characters", "_Base model"],
    'Mashup form': ["_First lyrics", "_Second lyrics", "_Mashup (copy it from here)"],
    'Add to playlist form': ["_Playlist", "Name for a _new playlist"],
    'Edit form': ["_Title", "_Lyrics"],
    'Upload form': ["_Title", "_Lyrics (optional)", "I _own this audio or have the rights to upload it, and I accept Suno's terms for uploads (suno.com/terms)"],
    'Playlist rename form': ["_Name", "_Description"],
    'Find a creator form': ["_Handle: the name after suno.com/@, or the whole link"],
    'Report form': ["_What happened, and what did you expect?", "_Email or other way to reach you (optional)",
                    "Include Tin Pan's _log (it has no passwords, and your Windows user name is taken out)"],
    'Suggestion form': ["_What would you like Tin Pan to do?", "_Email or other way to reach you (optional)"],
}


def access_key(label):
    m = re.search(r'_(.)', label)
    return m.group(1).lower() if m else None


def main():
    sys.stdout.reconfigure(encoding='utf-8')  # Romanian, Ukrainian and Spanish in a Windows console
    table = {}
    if os.path.exists(OUT):
        table = json.load(open(OUT, encoding='utf-8'))
    for batch in sys.argv[1:]:
        for k, v in runpy.run_path(batch)['T'].items():
            table[k] = list(v)
    problems = []
    used = keys_in_code()
    missing = [k for k in used if k not in table]
    problems += [f'missing: {k!r}' for k in missing]

    ph = re.compile(r'\{(\d+)(?:[,:][^}]*)?\}')
    for k, row in table.items():
        if len(row) != 3 or any(not r.strip() for r in row):
            problems.append(f'incomplete: {k!r}')
            continue
        for lang, text, forms in zip(LANGS, row, PLURAL_FORMS):
            if '|' in k and k.count('|') == 1 and '{0' in k:
                if len(text.split('|')) != forms:
                    problems.append(f'{lang}: {forms} plural forms needed for {k!r}')
                pieces = text.split('|')
            else:
                pieces = [text]
            want = sorted(set(ph.findall(k.split('|')[0])))
            for p in pieces:
                if sorted(set(ph.findall(p))) != want:
                    problems.append(f'{lang}: placeholders differ for {k!r}')
                if k.startswith(' ') != p.startswith(' ') or k.endswith(' ') != p.endswith(' '):
                    problems.append(f'{lang}: leading or trailing space differs for {k!r}')
                if k.count('_') == 1 and '_' in k and p.count('_') > 1:
                    problems.append(f'{lang}: more than one access key in {p!r}')

    groups = {}
    groups.update(xaml_groups())
    groups.update(command_groups())
    groups.update(v2_command_groups())
    groups.update(FORMS)
    for name, labels in groups.items():
        for i, lang in enumerate(['English'] + LANGS):
            seen = {}
            for label in labels:
                text = label if i == 0 else table.get(label, [label] * 3)[i - 1]
                key = access_key(text)
                if key is None:
                    continue
                if key in seen and seen[key] != text:
                    problems.append(f'{lang}, {name}: "{key}" is used by both {seen[key]!r} and {text!r}')
                seen.setdefault(key, text)

    unused = [k for k in table if k not in used]
    if sys.argv[1:]:
        ordered = {k: table[k] for k in used}
        with open(OUT, 'w', encoding='utf-8', newline='\n') as f:
            json.dump(ordered, f, ensure_ascii=False, indent=1)
            f.write('\n')
        print(f'wrote {len(ordered)} texts to {os.path.relpath(OUT, ROOT)}' + (f'; dropped {len(unused)} no longer used' if unused else ''))
    print('\n'.join(problems) if problems else 'all translations check out')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
