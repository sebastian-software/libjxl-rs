# Compare against the original C++ implementation

- Status: accepted
- Created: 2026-10-31
- Updated: 2026-11-02

Round trips within the Rust port can agree with each other while both differ
from libjxl. Tests therefore compile the pinned, unmodified C++ source into a
separate oracle executable and compare its observations with Rust.

The oracle's line protocol is test infrastructure. It calls original upstream
functions; it does not reimplement their algorithms. The Rust crate has no
C/C++ build or link dependency. `cpp-oracle` enables differential integration
tests and fails when the executable is unavailable. The repository check
command always runs this evidence, including debug and release Rust builds.

Compare encoded bytes and bit counts, decoded values, exact status codes,
consumed-bit counters, padding, truncation, and finalization behavior. Include
boundary values and deterministic generated inputs. Image-level conformance,
cross-platform behavior, and performance remain separate, future gates.

Build the complete original static codec library through its own CMake project,
with scalar Highway, Brotli, and skcms at upstream's submodule pins. This keeps
the reference's image entry points available without copying build definitions
or supplying replacement implementations. Disable upstream tools, tests, and
optional integrations; do not change codec routines. Generate export and version
headers through the original build. These dependencies belong exclusively to
the C++ oracle; normal Rust builds remain independent of them. Sanitizer flags
apply before adding the upstream project, covering the adapters, codec, and
built dependencies.

This requires a C++ toolchain and the reference's pinned dependencies for
the checks. Review the oracle whenever a source revision, compile configuration,
protocol, or ported boundary changes. Never turn missing oracle evidence into a
passing test.

The scalar Clang build retains upstream `lib/CMakeLists.txt`'s
`-fno-slp-vectorize -fno-vectorize` settings. Repository Rust builds mirror them
with `-C no-vectorize-slp -C no-vectorize-loops` in
[Cargo configuration](../../.cargo/config.toml). They affect floating-point
reductions: allowing auto-vectorization changed LZ77 cost rounding, and ASan
then selected a different reduction loop. Both ordinary and instrumented
oracles must use the upstream settings. Rust SLP also combined three DC
reciprocals and an unchanged copy into a four-lane division, computing the copy
as `value / 1`. That quieted a signaling NaN in release while C++ retained its
original stored bits. Preserve the original compile configuration in both
languages so raw copies remain bit-exact.
Keep exact float-bit comparisons;
do not compensate with relaxed tolerances or changed reference algorithms.

Compile the unchanged scalar Gaborish implementation through its own
`tools/oracle/gaborish.cc` observation unit at scoped `-O0`. Clang 21 x86's
ordinary and ASan/UBSan `-O3` objects commute different additions in the nested
Highway expression, choosing different competing NaN payloads. One native
arithmetic contract cannot simultaneously preserve those different objects.
The isolated unit retains the original scalar operator bodies and their explicit
float-to-double/product/float checkpoints; every sanitizer flag stays active.
The original codec library and existing fixture units keep their build options.
Link maps must bind the real GetGaborishStage and ProcessRow to this dedicated
unit; no stage/factory algorithm is supplied by the adapter. Review this numeric
configuration and its exact exceptional-value evidence after source, compiler,
target, linker, or instrumentation changes. It establishes no performance parity.

The scoped `-O0` setting does not by itself prove stable constructor operand
order: Clang 21 x86 plain code multiplies each normalized weight with the
reciprocal in the destination operand, while ASan/UBSan code commutes the first
two weight products and retains reciprocal-first for the third. Native captures
of the competing signaling/quiet filter-pair observations agree between both
complete original builds. Keep the complete ordinary and fully instrumented
original oracle comparisons on native hardware as separate gates. QEMU's x86
arithmetic selected the same larger
quiet-NaN payload in either operand order, so its passing exceptional-value
comparisons cannot establish native x86 payload parity. Captured native words
must come independently from those actual original stage/factory objects;
temporary probes or Rust outputs cannot supply expected words.

The frame fixture translation unit uses Clang/GCC `-fno-access-control` to
observe and seed the original private state at internal boundaries. This is
limited to the test adapter and does not change declarations, layouts, source
files, or runtime algorithms. Do not replace private methods with test versions
or rewrite source access specifiers. Omit source-indeterminate state until the
original algorithm initializes it. Observe writer bits through a padded scratch
copy so inspecting an intermediate result does not alter the real writer.

Frame fixtures follow the original caller's lifecycle: shared trees/tokens and
histograms belong to the non-streaming path; streaming uses local generic
streams. Internal callers must satisfy source preconditions before observation.
Undefined source inputs or invalid lifecycle combinations do not become passing
Rust behavior by suppressing sanitizer failures or changing reference routines.

The shared-state frame fixture calls the complete original ComputeEncodingData
for state/group observations, then independently calls EncodeFrameOneShot and
its real output processor for complete frame bytes. The native side calls its
translated fixed-profile ComputeEncodingData and independently its complete
one-shot frame boundary. The image fixture calls the original codestream-header
block and one-shot encoder, then the public C++ decoder on both source and
supplied native streams. It compares exact raw codestream bytes, original
integer samples, metadata, and auxiliary statistics. Complete Rust image
decoding remains a separate acceptance gate. Small DC-reader fixture spans use
real backing storage with
eight leading and trailing bytes while preserving their original span length
and contents. This keeps the source end-minus-eight pointer inside its backing
allocation and represents a non-null empty stream rather than the null-reader
sentinel. The source reader and its status/close algorithms stay unchanged.

Keep Quantizer's constructor/recomputation boundary stable in the reference.
Clang's ordinary release build inlined the default constructor's arithmetic
and reduced multiplier-by-one operations to stored copies. ASan's higher
inlining cost retained the RecomputeFromGlobalScale call and quieted signaling
NaNs. Assembly inspection verified both paths. Compile only the original
quantizer.cc with -fno-inline-functions so both reference variants execute the
source recomputation. CMake 3.18 is required for that source's directory-scoped
option. Rust marks RecomputeFromGlobalScale inline(never) for the same reason.
This pins one reproducible numeric configuration while keeping all original
source routines, scalar algorithms, and exact comparisons. It does not establish
performance parity. Review this boundary after compiler/source changes; do not
replace it with NaN-bit rewriting or relaxed comparisons.

Observe NoiseParams::HasAny through a separate reference translation unit with
-fno-unroll-loops. On AArch64, Clang O3 otherwise replaces the early-return loop
with a chain of FMAXNM instructions. A signaling NaN in the final lane makes the
reduction return false even when an earlier value exceeds the threshold; the
source loop returns true at that earlier value. Keep the original header and
method unchanged and retain all exceptional-value cases. This compile setting
is limited to the noise observation unit; it does not alter the codec library
or other fixtures. Review it with compiler/target changes.

The modular frame decoder fixture calls the original DecodeGlobalInfo and
MaybeDropFullImage on original encoder payloads. It explicitly seeds otherwise
indeterminate constructor flags/code scalars before comparing partial state.
Only globally decoded channels expose samples; the first oversized non-meta
channel ends that initialized prefix. The remaining channels retain real
allocations without observing their undefined source contents. Local generic
encoder streams exercise the no-shared-tree branch and metadata/channel shifts.
Repeated calls preserve existing owners and tables at the original checkpoints.
The fixture supplies frame metadata and the reader's initial bit position;
it does not claim complete frame/image decoding.

Render fixtures derive callback stages from the original abstract stage and
run the unchanged simple pipeline and image writers. They include upstream's
unmodified interpolation test stages; native fixture stages translate those
tests rather than replace production pipeline code. Callback probes supply
defined failures and dimension transitions to compare scheduling and partial
state. The adapter TU uses -fno-rtti to match upstream CMake's class hierarchy;
the original library omits base type information. This is limited to that
adapter, alongside its existing access-control option. No runtime algorithm or
source access declaration is changed. Observations omit undefined padding and
unfilled groups. Image setter fixtures initialize their input pixels before
testing ownership and post-assignment errors.

The dedicated XYB fixture translation unit includes pristine stage_xyb.cc and
its dec_xyb-inl.h helper and compiles with source-scoped -O0, alongside the
existing access-control/no-RTTI fixture options. Its original factories and
stage methods bind from this object; the archive does not supply a second XYB
implementation. Ordinary optimization commutes competing-NaN operands
differently after ASan inserts matrix-load checks. For example, a quiet-NaN
matrix coefficient multiplied by an independently produced canonical NaN
selects a different payload between the original optimized ordinary and
sanitized objects. A small noinline adapter alone does not remove this drift.
The isolated -O0 setting retains the original scalar expression operand order
for both configurations, including the actual stage, without replacing any
source routine or changing the original codec-library/other fixture settings.
All sanitizers and every raw-parameter/exceptional-value case remain enabled.
This is a reproducible numeric reference configuration, not a performance
comparison or a change to an existing source algorithm/optimization. Review it
with compiler/target/source changes; do not replace it with NaN rewriting or
relaxed comparisons. Ordinary and instrumented complete-image parity remain
separate required regression gates.

Select libc++ explicitly for the Linux Clang reference and install its
development packages in CI. The translated palette sort preserves the recorded
libc++ non-arithmetic specialization; silently selecting libstdc++ compares a
different algorithm with a different equivalent-element order. The complete
existing sort and codec corpus remains enabled for the selected library.
Other standard-library algorithms/configurations require separate translation
and evidence. This does not add a C++ dependency to the Rust library.

The frame-finalization fixture allocates backing storage for an empty group
through the original BitWriter::WithMaxBits without writing bits. Its vector
append calls memcpy even for a zero-byte span when other groups are nonempty;
Linux's nonnull declaration rejects a null source. Supply a real zero-length
span rather than suppress that sanitizer check or remove empty-group cases.
The original writer and codec source stay unchanged.

The AC quantization fixture calls the unchanged original DequantMatrices
library, matrix, cache, and decode methods through quant_ac.cc. Its scoped
-fno-access-control option observes initialized cache masks and offsets without
changing source declarations or layout. All numeric compiler and sanitizer
flags remain the same as the complete original library. Original AC encoding
and modular frame encoding generate local and real shared-tree RAW streams for
decoder comparisons; they do not implement a Rust encoder or runtime fallback.

Failed non-RAW table parsing can retain the old Library mode after writing a
different overlapping payload member. The AC fixture copies the initialized
union object representation with memcpy before printing its float words; it
does not load an inactive floating member. Source-indeterminate coefficient
pointers and uncomputed table slots remain unobserved. Empty reader fixtures
retain real padded backing while preserving their zero logical length. An empty
writer receives no zero-bit Write call and its unused span is not read. These
are source caller/lifecycle preconditions, with no sanitizer suppression.

Exceptional AC matrix cases keep geometry finite. A single distance band
bypasses FastPowf, allowing strict comparisons of NaN seed weights divided by
independent signed quiet/signaling NaN multipliers in DCT4/DCT4X8. AFV direct
weights and single-band submatrices exercise NaN words while its interpolation
bands stay finite. Do not feed NaN/negative bases to FastLog2f, whose original
source explicitly leaves those inputs undefined. Ordinary and fully instrumented
reference graphs remain separate gates for these unchanged operand expressions.

The dedicated dct.cc observation unit calls pristine scalar DCT/IDCT,
TransformToPixels, LowestFrequenciesFromDC, and strategy order methods. Compile
only this new unit with -fno-unroll-loops, retaining O3, default contraction,
and every sanitizer. Native AppleClang 21 AArch64 otherwise unrolls the ordinary
identity residual into FMSUB while its instrumented loop uses FMADD with a
negative finite scale, choosing different NaN signs. Both selected original
objects retain FMADD and agree on the complete focused corpus. Comparison with
the saved default ordinary adapter preserves every finite-input observation,
including 192 subnormal/cancellation/signed-zero requests and all initialized
coefficient, output, and scratch words. The exact original observations and
native mapping are recorded in ADR 0003. Disabling contraction was rejected
because it changed 21 finite identity requests; fast contraction was rejected
because it changed finite DCT/LLF results and retained instrumented NaN drift.
The original archive and existing fixtures keep their flags. Complete native
ordinary and instrumented graphs remain separate required gates; review the
configuration with compiler/source/target/instrumentation changes.

Observe pristine compressed_dc.cc through a dedicated vardct_dc.cc translation
unit compiled at scoped -O0 with all sanitizers active. Optimized native AArch64
ordinary and ASan objects commute competing NaN operands in adaptive smoothing
and produce different payloads from identical initialized inputs. The selected
ordinary/instrumented observers agree on all 1,322 captured fixture requests;
all 1,202 finite requests also agree with both unchanged optimized archives.
Retain every exceptional request and exact word. Link maps must bind the actual
public AdaptiveDCSmoothing/DequantDC and scalar implementations from this selected
unit for complete decoder observations too; an isolated observer alone does not
establish whole-image parity. The original archive and earlier fixture flags
remain unchanged. This preserves a reproducible source configuration without
claiming performance parity. Revisit after source/compiler/target changes.

Native x86_64 Linux dequantization exposed a second configuration-dependent
operand order in the new isolated DequantLane observer. In the same mode-4
request, the ordinary original B[56] is 0x7fc12345 while ASan/UBSan is
0xffc98765. Both original graphs choose 0xffe98765 for X[7], unlike the native
ARM64 residual-first observation. These are independent captures from run
41410474499; changing Rust alone cannot reproduce both B observations.

On x86_64 Linux, compile only dequant.cc with -O0, keeping its hash-checked
pristine scalar bodies, every sanitizer, all earlier adapters and actual decoder
archive objects unchanged. Native x86 correlation sums retain prediction-first
addition through an out-of-line binary32 boundary; ARM mappings stay unchanged.
The complete exact corpus remains mandatory. The Linux diagnostics additionally
recompile that identical adapter at its default -O3 and relink against the same
remaining objects without mutating the tested graph. Compare all 448 finite
requests against each default graph and require the selected ordinary/ASan
objects to agree on all 546 finite/exceptional requests. Assembly inspection is
supporting evidence; native CI is the acceptance gate. Review this scoped
configuration again after source, compiler, target or instrumentation changes.


Compile the pristine scalar EPF stage in its dedicated `epf.cc` observation unit
at scoped `-O0`, with every sanitizer active. Native ARM default optimized ordinary
and instrumented archive objects disagree on 107 of 3,672 identical row requests.
For request 1,299, pass one output word 61 is `0x7fe54321` ordinarily and
`0x7fc98765` under ASan/UBSan. A Rust operand mapping cannot reproduce both
objects simultaneously. The selected unit supplies the real public factories
and stage methods, including for complete-file observations; source routines and
the original archive remain unchanged.

The durable EPF reference audit relinks the exact adapter against each untouched
default archive, retaining every original compiler and sanitizer argument and
using temporary objects only. It verifies original factory bindings, requires all
4,152 finite-arithmetic rows to match both default graphs, and requires all 6,936
selected rows to agree between ordinary and instrumented graphs. Keep exceptional
float words exact and revisit this configuration with source/compiler/target or
instrumentation changes. This is a reproducible numeric reference configuration,
not performance evidence.


The pristine scalar SpotColorStage observation unit uses scoped `-O0` only on
Linux x86-64. Native ordinary and ASan/UBSan `-O3` original objects disagreed on
3,074 of the existing 91,072 requests, including different RGB NaN payloads
within one row. Both the active observation unit and untouched archive objects
demonstrated that disagreement. Native independently compiled pristine `-O0`
objects agreed on every request and matched both default graphs on all 34,946
finite-input and 368 no-arithmetic observations. The finite-input corpus retains
overflow/cancellation results; no-arithmetic cases retain raw exceptional words
through factory queries, rejected extra spans and empty valid rows.

`tools/spot_reference.py --audit --output <new-directory>` retains all actual
Rust-generated requests, actual factory/row link maps, default-archive and
independent selected objects, complete output words, and source/object hashes.
It requires ordinary/ASan agreement and every finite/no-arithmetic cross-default
comparison. The original archive, other units, sanitizer flags and ARM scalar
contraction keep their existing configuration. No strict-FP option is needed
for Spot. The dedicated native diagnostic workflow additionally runs Debug,
Release and Rust 1.85 against both selected graphs; those Rust results remain
a distinct acceptance gate from the original-source configuration audit.

The selected native source audit passed, while the original Rust Release build
still failed the exceptional-sample and repeated-row groups in both source
graphs. Debug and Rust 1.85 passed all five groups and four native regressions.
For the existing request `spot-sample 0 0 2143363909 2143363909 2143363909 0
4290917462 0`, both selected source graphs and independently rebuilt pristine
`-O0` graphs return RGB word `2143363909`; optimized Rust returned `4290917462`.
Linux x86-64 therefore retains each source multiplication and final addition
through a separate binary32 function boundary. The subtraction and literal
operand tree stay unchanged; ARM and other targets retain their existing
contraction mapping. No request or expected word changes.

Native Rust acceptance requires the complete unchanged corpus in Debug,
Release and Rust 1.85 against both selected source graphs after this mapping.
The diagnostic workflow explicitly enables Bash pipeline failure propagation
while collecting every configuration's log. A successful source audit or overall
workflow conclusion alone is insufficient: verify all six actual test results
and exact request counts before the full local gates or integration.
