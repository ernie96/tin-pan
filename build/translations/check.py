"""
Merges the translation batches into TinPan/Localization/translations.json and checks them:
every English text in the code has Romanian, Ukrainian and Spanish; placeholders match; plural
forms have the right count; and access keys (the letter after "_") are unique on each page,
menu and form, in every language.

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
    return [k for k in dict.fromkeys(keys) if re.search('[A-Za-z]{2}', k)]


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
    'Add to playlist form': ["_Playlist", "Name for a _new playlist"],
    'Edit form': ["_Title", "_Lyrics"],
    'Upload form': ["_Title", "I _own this audio or have the rights to upload it, and I accept Suno's terms for uploads (suno.com/terms)"],
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
