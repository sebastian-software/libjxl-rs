#!/usr/bin/env python3
"""Diagnose pristine SpotColorStage objects without selecting a configuration.

All requests come from the existing Rust fixture generators. Temporary links
call actual original stage factories; no pixel algorithm or expected word is
supplied here. Active objects, dependency files, binaries and pins stay intact.
"""
import argparse
from collections import Counter
from itertools import zip_longest
import gzip
import hashlib
import json
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
GROUPS = [
    'spot_factory_modes_settings_wrapping_and_guard_before_access',
    'spot_all_four_operand_special_words_and_nan_priorities',
    'spot_finite_rounding_subnormals_and_random_operand_words',
    'spot_rows_preserve_padding_ignored_channels_and_live_coefficients',
    'spot_real_multigroup_simple_pipeline_and_ordered_spot_sequence',
]
COUNTS = [80, 65536, 24832, 504, 120]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def function(source, name):
    found = re.search(r'^fn ' + re.escape(name) + r'\(.*?^\}', source, re.M | re.S)
    if not found:
        raise RuntimeError('Missing actual Rust request generator: ' + name)
    return found.group(0)


def requests(directory):
    source = (ROOT / 'tests/parity/spot_color.rs').read_text()
    parent = (ROOT / 'tests/parity.rs').read_text()
    constant, = re.findall(r'^const COLOR:.*$', source, re.M)
    code = ('use std::fmt::Write as _;\n'
            'fn compare(cases:Vec<String>){for case in cases{println!("{}",case);}}\n'
            + function(parent, 'next') + '\n'
            + '\n'.join(function(source, name) for name in ['words', 'exact'])
            + '\n' + constant + '\n'
            + '\n'.join(function(source, name) for name in GROUPS)
            + '\nfn main(){' + ''.join(name + '();' for name in GROUPS) + '}\n')
    path, binary = directory / 'requests.rs', directory / 'requests'
    path.write_text(code)
    subprocess.run(['rustc', '--edition=2024', str(path), '-o', str(binary)], check=True)
    cases = subprocess.check_output([str(binary)], text=True).splitlines()
    if len(cases) != sum(COUNTS):
        raise RuntimeError('Existing 91,072-request inventory changed')
    actual = Counter(case.split()[0] for case in cases)
    if actual != {'spot-query': 16, 'spot-guard': 64, 'spot-sample': 90368,
                  'spot-row': 504, 'spot-simple': 120}:
        raise RuntimeError('Actual request boundary counts changed: ' + str(actual))
    return cases


def finite_words(words):
    return all(int(word) & 0x7f800000 != 0x7f800000 for word in words)


def classify(case):
    """Classify reached operands, never change or exclude a source request.

    Finite-input samples include overflow/cancellation and all resulting words.
    Row modes 0/1/2 use only finite RPValue inputs. On reuse every actually read
    coefficient array must be finite. Mode 3 contains NaNs/infinities and is
    exceptional even with zero scale. Factory queries, rejected extra spans and
    valid empty spans perform no floating-point arithmetic; raw copies there
    must also remain exactly equal to both unchanged default graphs.
    """
    a = case.split()
    if a[0] in ('spot-query', 'spot-guard'):
        return 'no-arithmetic'
    if a[0] == 'spot-sample':
        return 'finite-input' if finite_words(a[3:9]) else 'exceptional-input'
    if a[0] == 'spot-row':
        owner, width, left, right, passes, mode = map(int, [a[2], a[7], a[8], a[9], a[11], a[12]])
        if width == 0 or left or right:
            return 'no-arithmetic'
        colors = a[13:13 + 4 * passes] if owner else a[3:7]
        return 'finite-input' if mode != 3 and finite_words(colors) else 'exceptional-input'
    if a[0] == 'spot-simple':
        count, mode = int(a[3]), int(a[5])
        # The source mutates each external array after factory construction.
        colors = [word for i in range(count) for word in a[7 + 9*i + 5:7 + 9*i + 9]]
        return 'finite-input' if mode != 3 and finite_words(colors) else 'exceptional-input'
    raise RuntimeError('Unclassified actual source boundary: ' + a[0])


def binding(contents, selected):
    def relevant(line):
        return ('GetSpotColorStage' in line or
                ('SpotColorStage' in line and 'ProcessRow' in line))
    if sys.platform == 'darwin':
        entries = dict(re.findall(r'^\[\s*(\d+)\]\s+(.+)$', contents, re.M))
        rows = []
        for line in contents.splitlines():
            if not relevant(line) or 'FDE for:' in line:
                continue
            match = re.search(r'\[\s*(\d+)\]\s+', line)
            if match and match[1] in entries:
                rows.append((entries[match[1]], line))
    else:
        rows, current = [], ''
        for line in contents.splitlines():
            if '.o' in line and ('CMakeFiles/' in line or '.a(' in line):
                current = line
            if re.match(r'^\s+0x[0-9a-f]+\s+', line) and relevant(line):
                rows.append((current, line))
    expected = 'CMakeFiles/jxl_oracle.dir/spot_color.cc.o' if selected else 'stage_spot.cc.o'
    if len(rows) != 2 or any(expected not in origin for origin, _ in rows):
        raise RuntimeError('Actual pristine Spot factory/row binding changed: ' + str(rows))
    return rows


def build_reference(build, directory, options=None, default=False):
    directory.mkdir(parents=True)
    obj = directory / 'CMakeFiles/jxl_oracle.dir/spot_color.cc.o'
    obj.parent.mkdir(parents=True)
    entries = json.loads((build / 'compile_commands.json').read_text())
    entry, = [e for e in entries if Path(e['file']).resolve() == ROOT / 'tools/oracle/spot_color.cc']
    command = shlex.split(entry['command'])
    command[command.index('-o') + 1] = str(obj)
    if default:
        original = (ROOT / 'tools/oracle/spot_color.cc').read_text()
        old = '#include "lib/jxl/render_pipeline/stage_spot.cc"'
        if original.count(old) != 1:
            raise RuntimeError('Pristine source include changed')
        adapter = directory / 'default-adapter.cc'
        adapter.write_text(original.replace(old, '#include "lib/jxl/render_pipeline/stage_spot.h"'))
        command[command.index('-c') + 1] = str(adapter)
    for i, flag in enumerate(command):
        if flag in ('-MF', '-dependency-file'):
            command[i + 1] = str(directory / 'spot_color.cc.o.d')
        elif flag.startswith('-MF'):
            command[i] = '-MF' + str(directory / 'spot_color.cc.o.d')
        elif flag.startswith('-dependency-file='):
            command[i] = '-dependency-file=' + str(directory / 'spot_color.cc.o.d')
    command.extend(options or [])
    subprocess.run(command, cwd=build, check=True)
    link = shlex.split((build / 'CMakeFiles/jxl_oracle.dir/link.txt').read_text())
    link[link.index('CMakeFiles/jxl_oracle.dir/spot_color.cc.o')] = str(obj)
    binary, mapping = directory / 'jxl_oracle', directory / 'selected.map'
    link[link.index('-o') + 1] = str(binary)
    link.append('-Wl,-map,' + str(mapping) if sys.platform == 'darwin' else '-Wl,-Map,' + str(mapping))
    subprocess.run(link, cwd=build, check=True)
    bound = binding(mapping.read_text(), not default)
    dump = ['xcrun', 'llvm-objdump', '-d', '--demangle'] if sys.platform == 'darwin' else ['objdump', '-d', '-C']
    (directory / 'object.asm').write_text(subprocess.check_output(dump + [str(obj)], text=True))
    return binary, {'compile': command, 'link': link, 'bindings': bound,
                    'object_sha256': digest(obj), 'binary_sha256': digest(binary)}


def observe(binary, cases, path):
    with gzip.open(path, 'wt') as output:
        for begin in range(0, len(cases), 128):
            chunk = cases[begin:begin+128]
            result = subprocess.run([str(binary)], input='\n'.join(chunk)+'\n',
                                    capture_output=True, text=True, check=True)
            lines = result.stdout.splitlines()
            if len(lines) != len(chunk) or result.stderr:
                raise RuntimeError('Original request/response or stderr changed: ' + result.stderr)
            for line in lines:
                output.write(line+'\n')


def compare(left, right, classes, finite_only=False):
    bad, compared = [], 0
    with gzip.open(left, 'rt') as a, gzip.open(right, 'rt') as b:
        for i, (x, y) in enumerate(zip_longest(a, b)):
            if x is None or y is None:
                raise RuntimeError('Original observation line counts differ')
            if finite_only and classes[i] == 'exceptional-input':
                continue
            compared += 1
            if x != y:
                xx, yy = x.split(), y.split()
                bad.append({'index': i, 'lengths': [len(xx), len(yy)],
                            'first_deltas': [(j, p, q) for j, (p, q) in enumerate(zip(xx, yy))
                                             if p != q][:16]})
    return {'compared': compared, 'mismatches': bad}


def active_files(builds):
    paths = []
    for build in builds:
        paths += [build/'jxl_oracle', build/'compile_commands.json',
                  build/'CMakeFiles/jxl_oracle.dir/link.txt']
        paths += list(build.rglob('*.o')) + list(build.rglob('*.o.d')) + list(build.rglob('*.a'))
    return {str(p): digest(p) for p in paths}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    builds = [ROOT/'target/oracle', ROOT/'target/oracle-asan']
    before = active_files(builds)
    source = ROOT/'upstream/libjxl/lib/jxl/render_pipeline/stage_spot.cc'
    pins = {name: subprocess.check_output(['git', '-C', str(ROOT/name), 'rev-parse', 'HEAD'], text=True).strip()
            for name in ['upstream/libjxl', 'upstream/libjxl/third_party/highway',
                         'upstream/libjxl/third_party/brotli', 'upstream/libjxl/third_party/skcms']}
    expected = json.loads((ROOT/'upstream.json').read_text())['revision']
    if pins['upstream/libjxl'] != expected:
        raise RuntimeError('Upstream source revision differs from the repository pin')
    for name in ['highway', 'brotli', 'skcms']:
        entry = subprocess.check_output(['git', '-C', str(ROOT/'upstream/libjxl'),
                                         'ls-tree', 'HEAD', 'third_party/'+name], text=True)
        if pins['upstream/libjxl/third_party/'+name] != entry.split()[2]:
            raise RuntimeError('Pinned source dependency changed: '+name)
    if subprocess.check_output(['git', '-C', str(ROOT/'upstream/libjxl'), 'status', '--porcelain',
                                '--ignore-submodules=none'], text=True).strip():
        raise RuntimeError('Pristine source/dependency tree is dirty')
    cases = requests(output)
    classes = [classify(case) for case in cases]
    with gzip.open(output/'requests.json.gz', 'wt') as f:
        json.dump([{'request': case, 'classification': c} for case, c in zip(cases, classes)], f)
    report = {'platform': platform.platform(), 'pins': pins, 'source_sha256': digest(source),
              'rust_generator_sha256': digest(ROOT/'tests/parity/spot_color.rs'),
              'groups': dict(zip(GROUPS, COUNTS)), 'requests': len(cases),
              'classification_counts': dict(Counter(classes)), 'objects': {}, 'comparisons': {},
              'active_files_before': before, 'configuration_selected': False}
    for graph, build in zip(['ordinary', 'sanitizers'], builds):
        observe(build/'jxl_oracle', cases, output/f'active-{graph}.txt.gz')
        binary, record = build_reference(build, output/f'default-{graph}', default=True)
        report['objects'][f'default-{graph}'] = record
        observe(binary, cases, output/f'default-{graph}.txt.gz')
    def record(label, left, right, finite=False):
        result = compare(output/left, output/right, classes, finite)
        report['comparisons'][label] = result
        print(f'{label}: {result["compared"]} compared, {len(result["mismatches"])} mismatches', flush=True)
        return result
    record('active ordinary/ASan', 'active-ordinary.txt.gz', 'active-sanitizers.txt.gz')
    record('default ordinary/ASan', 'default-ordinary.txt.gz', 'default-sanitizers.txt.gz')
    options = [('O0', ['-O0'])]
    candidate = 0
    while candidate < len(options):
        label, flags = options[candidate]
        for graph, build in zip(['ordinary', 'sanitizers'], builds):
            binary, metadata = build_reference(build, output/f'{label}-{graph}', flags)
            report['objects'][f'{label}-{graph}'] = metadata
            observe(binary, cases, output/f'{label}-{graph}.txt.gz')
        pair = record(label+' ordinary/ASan', f'{label}-ordinary.txt.gz', f'{label}-sanitizers.txt.gz')
        for graph in ['ordinary', 'sanitizers']:
            for default in ['ordinary', 'sanitizers']:
                record(label+f' finite/no-arithmetic {graph}/default-{default}',
                       f'{label}-{graph}.txt.gz', f'default-{default}.txt.gz', True)
        if label == 'O0' and pair['mismatches'] and platform.system() == 'Linux' and platform.machine().lower() in ('x86_64', 'amd64'):
            # Only actual source O0 disagreement justifies this second source
            # experiment. Preserve every sanitizer and target option.
            options.append(('strict-O0', ['-O0', '-ffp-model=strict']))
        candidate += 1
    after = active_files(builds)
    if before != after:
        raise RuntimeError('An active original object/binary/flag/dependency file changed')
    if digest(source) != report['source_sha256'] or digest(ROOT/'tests/parity/spot_color.rs') != report['rust_generator_sha256']:
        raise RuntimeError('Original stage or actual Rust fixtures changed during diagnostics')
    for name, revision in pins.items():
        if subprocess.check_output(['git', '-C', str(ROOT/name), 'rev-parse', 'HEAD'], text=True).strip() != revision:
            raise RuntimeError('A source/dependency pin changed during diagnostics')
    report['active_files_unchanged'] = True
    report['artifact_hashes'] = {str(p.relative_to(output)): digest(p)
                                for p in output.rglob('*') if p.is_file()}
    (output/'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print('All 91,072 actual fixture requests retained. No reference configuration selected.', flush=True)


if __name__ == '__main__':
    main()
