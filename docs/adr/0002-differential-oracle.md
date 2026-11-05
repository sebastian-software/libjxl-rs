# Compare against the original C++ implementation

- Status: accepted
- Created: 2026-10-31
- Updated: 2026-11-05

Round trips within the Rust port can agree with each other while both differ
from libjxl. Tests therefore compile the pinned, unmodified C++ source into a
separate oracle executable and compare its observations with Rust.

The oracle's line protocol is test infrastructure. It calls original upstream
functions; it does not reimplement their algorithms. The Rust crate has no
C/C++ build or link dependency. `cpp-oracle` enables differential integration
tests and fails when the executable is unavailable. The repository check
command always runs this evidence.

Run the differential tests in the `checked` Cargo profile: release
optimizations with overflow checks and debug assertions enabled. Optimized code
keeps the long corpus fast, and the checks still catch an omitted wrapping
operation or a violated debug assertion. Plain release only removes those
checks, so it is compiled but not tested again, except for test files with a
`cfg(not(debug_assertions))` variant. The minimum Rust version is a compile
check of all targets. One pass runs against each oracle graph: the ordinary
graph is the reference behavior, and the ASan/UBSan graph detects undefined
behavior in the original per case. The audits keep proving that both graphs
give identical results for defined cases.

Native Linux and macOS CI remains the acceptance gate, in two tiers. A fast
tier runs for every pull request and on `main`: the static checks and the
ordinary Linux oracle in parallel partitions, reusing an oracle build cached
under its exact inputs. It is the required check before a change lands. The deep
tier runs both oracle graphs and every audit in parallel jobs, building each
oracle from source. It runs nightly on `main` on Linux and macOS, and on demand
on Linux (macOS only when the run sets `macos`), so a regression that only the
instrumented graph, macOS, or an audit detects can reach `main` for up to a day.
A failed nightly run opens or updates an issue, and the fix lands forward. Start
the deep tier on a branch before merging a change that touches an oracle, a
sanitizer, or audit inputs.

Until the first release, hosted macOS jobs do not gate pull requests. The
organization shares few hosted macOS runners, and a deep gate waited about 40
minutes per macOS job while its Linux jobs started at once; both defects the
deep gate found on 2026-11-05 were Linux-only. Development happens on macOS
ARM64, so a change that touches ported code lands only after the full local
`python3 tools/check.py` passes on macOS — with `--sanitizers` when it touches
an oracle, a sanitizer setting, or audit inputs — and the pull request records
the command and its result. A local run is not hosted evidence: the nightly
deep gate supplies that, and a release or an acceptance claim for macOS needs a
complete hosted macOS deep gate on the exact revision. Revisit this when macOS
runners stop being scarce or before the first release.

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

Upstream test inventories run as differential histories on the inputs their
own pinned helpers build. Helpers such as CreateTestJXLCodestream,
GetTestHeader, AnalyzeCodestream, GetSomeTestImage and test::EncodeFile are
extracted verbatim and hash checked, without gtest: an adapter supplies
expectations that throw, the pinned testdata bytes instead of file readers, and
a genuine cjpeg file where a helper would call the extras libjpeg encoder,
which the oracle does not build. A check that compares output with the encoder
input (Butteraugli, pixel distances) is replaced by exact equality with the
original decoder's output, not by a tolerance. A history that sets a parallel
runner runs on both decoders with the same runner and thread count: the
original's thread or resizable runner, and its Rust translation. Which thread
runs which task, and in which order, is nondeterministic. These histories
therefore compare statuses, events, getters and outputs through caller
buffers, including callback output written into a caller buffer at its stride.
For these histories, the original with a runner must also reproduce its own
runner-free history exactly. That history compares every callback call in
order as well. The multithreaded callback's init arguments depend on the
runner and its thread count, not on the schedule. They compare for explicit
thread counts, together with a check that each run call's thread id is below
the latest init's count. Default thread counts follow the host's reported
concurrency, which Rust and libc++ may report differently, so histories that
observe thread counts set them explicitly. The runners themselves are tested
for the properties of `thread_parallel_runner_test.cc`, not for one schedule.

The exported decoder C ABI is compared from the C side. One driver, compiled
against the pinned public headers only, replays the public decoder histories
with the same driver code as the original oracle. It is linked twice: against
the original archive and against the Rust static library. Both also link the
original default CMS library for JxlGetDefaultCms, which decode.h does not
declare; the original variant also links the original `jxl_threads` library,
whose runner functions the Rust library exports itself. Every history that
Rust replays in-process, with or without a parallel runner, also runs through
both executables, which must reproduce the original oracle's observation
exactly. A C unit of the driver measures every public struct
layout and enumerator and takes the address of every exported function. The
oracle build runs Cargo for the static library, with the profile and features
of the checked tests so that both share one compilation; Cargo decides
whether it is current, so a restored oracle cache relinks against the current
Rust sources. This is the only oracle program that links Rust code; the crate
itself still has no C or C++ build or link dependency.

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

Observe pristine `alpha.cc` through its dedicated translation unit with scoped
`-O0 -fno-inline-functions`. On native x86_64 Linux, additionally use
`-ffp-model=strict`; retain the original contraction configuration on ARM.
All six actual public alpha symbols, including complete-file calls, bind to
this exact source unit. Source bodies, archive flags and sanitizers stay intact.

The earlier no-inline configuration retained ARM std::max's original comparison
instead of optimized FMAXNM, but native Linux still exposed competing NaN drift.
Neither disabling unrolling nor O1 stabilized its ordinary/instrumented graphs;
O0 alone retained twelve mismatched requests. Actual strict-O0 native source
objects agree on all 1,928 diagnostic requests, and all 1,632 finite requests
match both untouched optimized graphs in
[native run 41446120185](https://github.com/sebastian-software/libjxl-rs/actions/runs/41446120185).
Strict-O3 still differs on 68 requests. Native disassembly confirms that strict-O0
retains the literal outer subtraction in new alpha and foreground-first addition;
non-strict O0 can negate the product and add one, changing a NaN sign. This source
configuration resolves an observed ambiguity rather than defining NaN bits.

ARM's selected O0 source objects also preserve every finite default word.
Applying strict FP there changes 272 finite requests by disabling source FMA
contraction, so that configuration is excluded on ARM. Rust retains the selected
source contraction on ARM and literal separately rounded operations on x86.
Local out-of-line arithmetic boundaries retain operand order without assembly,
NaN bit rewriting or additional target-feature assumptions.

`tools/alpha_reference.py` verifies the exact target configuration and all public
symbol bindings. It requires 1,944 selected finite/exceptional observations to
agree across ordinary and fully ASan/UBSan graphs, including 24 independent RGBA,
weighted-add and multiply requests with competing operands. All 1,632 finite
requests must match both untouched default archive graphs. Temporary relinking
removes only the selected object. The separate diagnostic retains actual native
object disassembly, binding maps, exact requests and word deltas; candidates never
replace active source objects. Revisit after source/compiler/target/instrumentation
changes. This establishes numeric parity, without a performance claim.

A separate original-source boundary probe preserves all 24 zero-width blending
requests whose aliased extra-channel weighted-add calls memcpy with a null
temporary destination at `alpha.cc:70`. Native Linux UBSan diagnoses that call
even when the byte count is zero. Each instrumented Linux probe must positively
report that exact diagnostic. ARM and ordinary executions are retained as
platform observations; successful execution does not establish defined pixel
semantics. These requests have no C++/Rust pixel comparison. The remaining 360
blend requests, including 72 zero-width cases, retain exact parity, and direct
zero-count alpha calls use non-null backing.

The tree corpus also retains every one of its original 1,728 requests. A native
Linux instrumented run exposed the same alpha.cc:70 boundary during actual
AddOneRow/stage rendering. Original decoded positions, reference rectangles and
blendings independently predict exactly 48 aliased zero-size calls: one extra
channel with modes 5/6, or three extra channels with modes 3/4, render modes 1/2,
and the x=8 zero-width geometry, across all three prefixes and both association
settings. Preserve the other 1,680 requests byte-for-byte in exact parity, and
execute these 48 in individual original-only subprocesses alongside the previous
24 probes. A recognized diagnostic must be the complete non-recovering
alpha.cc:70 null-argument report; reject other stderr or failures.

tools/patches_boundaries.py extracts the actual Rust fixture generator, obtains
its entropy streams from the original encoder, and independently derives the
boundary set from original decoded metadata. It executes all 1,728 requests
separately in both original graphs and saves full stdout, stderr, statuses and
source-trigger evidence. Native Linux instrumentation must establish precisely
the predicted 48 diagnosed requests; successful ARM executions do not confirm
their pixel semantics. The prediction remains distinct from native confirmation
until that census completes. Every defined ordinary/instrumented observation
must remain exact. No source fix, sanitizer suppression, blanket filter or
silently skipped request is introduced.

The dedicated pristine `noise_stage.cc` unit uses scoped `-O0`, retaining every
sanitizer. Native ARM optimized ordinary and ASan/UBSan stages disagree on 66 of
756 original row requests. In the first competing-NaN request, Y/B sample words
are `0x7fe11111` ordinarily and `0xffe22222` under instrumentation. The selected
unit supplies the real stage factories/methods to component and complete-file
calls; noise generation/decoding and the original archive retain their flags.
`tools/noise_reference.py` recompiles that identical unit with only its scoped
`-O0` removed and relinks temporary graphs without replacing active objects.
All 504 finite-input rows must match both default graphs and all 1,134 selected
rows must agree. Exact exceptional words remain mandatory; no old comparison,
expected word, sanitizer or source algorithm changes. Review the configuration
with source/compiler/target changes. This establishes no performance parity.


Native Linux x86_64 chroma diagnostics (run 41445638463, Clang 21.1.8) retain
all 4,842 original requests and add 720 finite tiny-value, cancellation, and
signed-zero rows. The unchanged optimized ordinary and ASan/UBSan archives
disagree on 181 of these 5,562 initialized requests, including 104 simple
pipelines. Independent pristine stage_ycbcr.cc and stage_chroma_upsampling.cc
objects at -fno-unroll-loops or -O1 retain all 181 disagreements. At -O0 the two
original graphs agree on every request, and all 4,592 finite requests match
both unchanged optimized archive graphs exactly.

Only on Linux x86_64, compile two dedicated pristine stage translation units at
scoped -O0. Their real public factories also serve complete-file callers. Keep
the original archives, earlier adapters, contraction settings, and every
sanitizer unchanged. Other platforms retain the original archive binding:
native ARM ordinary and instrumented archives already agree, while selecting
ARM -O0 would change 182 exceptional observations despite finite equivalence.
The existing ARM captured cases and words remain unchanged.

tools/chroma_reference.py --audit verifies this platform-specific binding,
records object symbols, link commands, source/object/archive hashes, and saves
the unchanged default and selected observations from both graphs. Require all
5,562 selected requests to agree across instrumentation, all selected Linux
x86_64 observations to match independent pristine -O0 objects, and every
selected observation on other platforms to match the untouched default. All
4,592 finite observations must also match both unchanged default graphs.
Native diagnostics and acceptance run on Linux x86_64 and macOS ARM; final
Rust parity remains a separate gate. This configuration makes no performance
claim and must be reviewed after source, compiler, target, or instrumentation
changes.

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

Native Rust acceptance requires the complete unchanged corpus against both
selected source graphs after this mapping. At the time, that meant Debug,
Release and Rust 1.85; it now means the optimized `checked` profile above, which
exercises the optimized code generation that exposed this difference.
The diagnostic workflow explicitly enables Bash pipeline failure propagation
while collecting every configuration's log. A successful source audit or overall
workflow conclusion alone is insufficient: verify all six actual test results
and exact request counts before the full local gates or integration.

Complete extra-channel fixtures call actual WriteCodestreamHeaders, EncodeFrame,
DecodeFrame, ConvertToExternal and ConvertChannelsToExternal. The selected
Spot/alpha public symbols serve these complete-file callers too. The independent
source audit verifies their link bindings and every source/dependency pin,
reproduces complete fixture bytes, retains actual native final-word captures,
and compares all complete/partial observations across ordinary and ASan/UBSan
graphs without changing active objects. Mixed per-channel sampling fixtures
rewrite only the original frame header/TOC on genuine 1x1 payloads; a zero-change
writer control reproduces original bytes and every channel's modular dimensions
remain 1x1. Reserved, unknown and invalid types use explicit finite bit mutations
of a genuine optional-channel file, selected by the actual source metadata
parser; their rejected type, status, consumption and initialized partial metadata
are observed rather than assigning those values new semantics.

Float16 ConvertChannelsToExternal stores scanlines through a typed
Span<hwy::float16_t>, requiring every row pointer to retain two-byte alignment.
The new padding-three export fixtures expose a real undefined source store on
row one, rather than codec rejection. Preserve all 888 original requests in an
independent original-only census, positively require the 528 actually reached
nonrecovering span.h:59 UBSan misalignment diagnostics, and retain the 360 defined
single-row/decode-error requests in exact parity. Add aligned padding-four
controls for every fixture, channel count and unpremultiplication choice. Keep
ordinary captures for all original requests without assigning semantics to
undefined writes. Uint16 output uses StoreLE16 byte stores and Float32 uses
byte-order memcpy stores, so their odd-stride requests remain defined and
unchanged. No sanitizer flag or source algorithm is altered.

Native [run 41459406348](https://github.com/sebastian-software/libjxl-rs/actions/runs/41459406348) confirms the corrected alpha and patch component on Linux and macOS. The independent Linux census executes every original tree request in each graph: ordinary execution diagnoses none; full ASan/UBSan diagnoses exactly the 48 predicted calls at alpha.cc:70. All 1,680 defined observations stay exact. This confirms the recorded source boundary and configuration; combined decoder integrations still need their own gates.


The complete output-stage observer calls the original public
GetWriteToOutputStage, not a forwarding bundle/DC writer. An independent map
requires that live factory and the actual scalar ProcessRow, PrepareForThreads,
GetChannelMode, GetName and destructor bind to the pristine stage_write.cc
include, and that manager initialization/allocation/free/aligned creation bind
to memory_manager_internal.cc in the original archive. The scalar helper
factory may be inlined or dead-stripped; it is not falsely claimed as a separate
live binding. Both optimized graphs retain their original numerical flags and
all nonrecovering sanitizers. Only the data-only observer uses scoped access
visibility and RTTI flags. Compiler identity, actual libc++ version and reverse
vector destruction order are recorded on Linux and macOS.

All 4,004 defined output/manager requests compare the complete initialized wire
observations, including actual callback data/opaque/owner identity, allocation
and failure order, strides, padding, tails, retry behavior and cleanup. Nine
immutable independently captured original cases additionally run without the
C++ feature. Original-only boundary probes do not invoke Rust on undefined
source calls: depth 31 reaching 2^31 is positively diagnosed at Highway
base.h:2459:27, and oversized uint32 shifts at stage_write.cc:551. Every first
introduced depth-31 request remains accounted for: 96 initial defined requests
stay exact; 96 synthesized-alpha requests retain individual positive diagnostics.
Real-alpha controls add the corresponding defined two/four-channel coverage.
All 48 new defined requests from the second extension remain exact as well.
The full 560-request source-only census also retains the explicit value-one
and oversized-shift probes, with per-request stdout/stderr and classification.
No blanket depth exclusion, expected-word rewrite, sanitizer relaxation or
upstream-source change is permitted. Review this boundary after source,
compiler, target or instrumentation changes.


The decoder signature/box-header observer compiles five exact source fragments
and calls the actual archive JxlSignatureCheck. Every generated fragment must
match its complete pinned source body, notice and original line mapping. The
audit checks exact original/observer optimization and nonrecovering sanitizer
options and resolves executable code addresses to their live archive/adapter
objects. Dead symbols and unwind rows cannot establish those bindings; compiler
cold suffixes are distinct symbols, not duplicate definitions of the entry
point. Relinked audit binaries do not replace the active oracle graph.
The skcms transfer/matrix boundary calls unchanged public functions from the
original CMS archive, with a scoped x86 concat observer described below. The
observer contains raw-word protocol and public calls, including actual
same-pointer inversion and null destinations. The original deprecated HLGish
shim reaches the archive's scaled constructor. Maps verify all 16 original
entry points in skcms.cc. Both optimized archive graphs retain their numerical
options and full nonrecovering instrumentation.

Native Linux Clang O3 ordinary and ASan concat objects disagree on 64 of the
432 new paired-NaN requests. Preserve every old request and the complete
ordinary/instrumented investigation. Observe this single complete original
function in a hash-verified, symbol-renamed extraction at scoped -O0. Its body,
source line mapping, types and instrumentation stay unchanged; the original
archive and every other source unit keep their options. The independent native
optnone investigation agrees across all 207,144 requests; each graph differs
from its prior O3 observations on 48 new paired-NaN requests only, with no
changed finite word or prior request. Retain those changes and original captures
rather than replacing expectations with Rust-derived words. Native admission
must bind the extracted body to its live object, reproduce the independent
captures, and compare every old concat observation with the unchanged archive.
Rust preserves the observed scalar multiplication/addition checkpoints. ARM
keeps the original archive's contracted operations and operand order. This
reference configuration establishes no performance parity and must be reviewed
after compiler, target, source or instrumentation changes.

Transfer coverage retains the initial 86,696 defined requests and adds 8,262
defined exceptional-parameter cases and 24,310 actual in-place inversions. The
complete original exceptional census retains all 8,624 requests: 362 positively
reach the NaN-to-int32 conversion in skcms_internals.h:149 and have no defined
Rust result. Every original-only request is recorded individually in both
source graphs; no parameter-word class is silently dropped or normalized.

Matrix coverage retains every one of the initial 71,226 source requests,
including the four initially failing infinity/NaN-sign observations. Its binary
fixture encodes input words only, with the original text hash in its manifest.
Another 16,200 paired exceptional-entry inversions and 18 standard-primary/null
output calls remain exact comparisons. Expected results always come from the
actual original functions. Source assembly records the original ARM contraction
and pre-negation checkpoints; Rust preserves those checkpoints explicitly.
Native Linux/macOS validation remains required before accepting this boundary.
ICC parsing, nonlinear curve fitting, pixel transformation, JXL default CMS and
complete-image profile admission remain separate source closures.


### skcms nonlinear fitting source context

Public ApproximateCurve, MaxRoundtripError and AreApproximateInverses bind to the
unchanged complete CMS archive. Direct observations of rg_nonlinear,
gauss_newton_step, max_roundtrip_error_checked and fit_nonlinear use a separate
pristine skcms.cc include executable. Its actual original compiler command is
retained, changing only the test observation TU/output and adapter-only defines;
all original optimization and nonrecovering sanitizer flags remain active. The
original archive supplies only its baseline transform dependency to that helper.
Actual live code addresses must map to the expected objects, excluding dead
symbols and unwind FDEs. The complete public request subset must agree between
archive and whole-include contexts and between ordinary/instrumented originals.

The ARM degenerate two-point nonlinear segment contracts the final subtraction
in `b = eval_curve(...) - a * index * dx`: the first product rounds and the
scaled subtraction uses FMADD. The Rust mapping preserves those checkpoints.
All five initially differing requests remain in the input inventory, including
error-slot aliases; source flags, expected words and finite comparisons do not
change. This establishes numeric equivalence for the recorded source context,
not performance parity. Native target acceptance remains required.

The transfer/matrix source audit captures exactly its two registered operation
families. Curve-fitting and parser audits configure their own original private
helpers independently. Adding another skcms family must not make an existing
audit accidentally run that family's tests without its required observer, or
change the retained requests and observations of the original audit.
The skcms parser observer calls public functions from the untouched original
CMS archive. Private curve helpers use a separate executable that includes the
complete pristine skcms.cc. Its compile command is copied from that actual
archive object's command, replacing only the input translation unit and output
object paths. Compiler, target, optimization, numeric settings and nonrecovering
sanitizers stay identical. Link maps must bind public parser/getter symbols to
the archive, retain the private original bodies in the separate helper, and
exclude a duplicate archive skcms.cc object from that helper. Every public
request is also compared in the pristine helper's whole-Parse compilation
context, because private arithmetic can inline differently inside the parser.

Profile observations include initialized fields and retained non-pointer curve
words on failures, borrowed pointer relationships, native ABI layout, getter
boundary behavior, explicit profile zeroing, static pointer identity and setter
self-aliases. Original ICC files remain in the corpus alongside generated tag,
LUT, priority, malformed and truncation cases. Large-count source-only requests
require the exact original-file/line arithmetic diagnostic before any payload
bounds error. Their same-count defined controls remain exact comparisons.
No source, sanitizer option, existing case or expected source word is relaxed.

Parser source-binding verification cross-checks each actual executable nm code
address against its expected live object and text range. Mach-O dead symbols and
unwind FDE rows cannot establish a binding. GNU local helper addresses must lie
inside the directly linked pristine-include object's actual text extent. Exact
optimization and nonrecovering sanitizer tokens are checked in both graphs.


### skcms baseline transform source closure

The transform observer calls the actual archive Transform, destination-usability
and approximate-profile/TRC APIs. A separate pristine skcms.cc include observes
private selection, destination preparation and reference CICP helpers, and calls
the original linked baseline::run_program for direct stages. Both contexts use
the original O3/DNDEBUG commands and complete nonrecovering ASan/UBSan flags.
The audit compares all public requests between both compilation contexts and
retains ordinary/instrumented observations independently. It verifies live code
addresses and archive/observer object ownership, exact private compile-command
substitution, actual baseline-object disassembly and complete format/op names
against the pinned headers. On x86_64 Clang it additionally requires the
enabled original HSW/SKX engine objects with their exact ISA options and live
bindings, and retains their disassembly. Elsewhere it requires both
SKCMS_DISABLE definitions.

The 151,592 transform inputs supplement all inherited transfer, fitting and
parser requests. They include all 42 formats, alpha combinations, zero/tail
lengths, same-address and partial overlap, actual original ICC files, generated
profiles, malformed guards, CICP priority and PQ/HLG parameters, profile mutation
and failure states, direct stages/CLUT grids, all half words and integer rounding
neighbors. Genuine uninitialized C++ and Rust contexts exercise source-defined
g-only classification, per-op TF fields and dimension-prefix grid points.
Whole inactive objects are never materialized merely to compare them.

A profile's previous pointer words can overlap a later active parametric
function. Observer pointer normalization applies separately to the old and new
active branches before computing representation changes. It normalizes only
address identity/borrowed offsets; it does not mask or canonicalize initialized
post-call function words. Failure snapshots retain every initialized field and
partially retained pointer state.

The initial 11,448 preflight requests and 148 initial numeric failures remain
regressions. Actual ARM assembly establishes rounded m1*g followed by the
original m0*r and m2*b contractions, XR packing and Lab contraction checkpoints.
Later exact agreement does not replace those requests or change source flags.
Existing transfer/parser source-UB censuses remain independent and unchanged.
Native Linux/macOS and complete JXL ICC/HDR files remain acceptance boundaries;
a passing local baseline helper suite does not complete default CMS integration.

The private curve selector retains signed int channel counts and its original
post-decrement condition. Negative counts other than INT_MIN perform no curve
or output access, including high-bit u32 A2B/B2A counts converted to int while
preparing a zero-pixel program. Same-path defined controls retain unknown curve
and CLUT payloads. Three direct/A2B/B2A INT_MIN requests require the actual
nonrecovering source skcms.cc:2536 signed-decrement diagnostic; ordinary
observations are retained without becoming Rust expected values. No broader
high-bit count range is excluded. All earlier numeric requests remain paired.


## Default CMS archive and private contexts

Compile the default CMS public observer with the actual selected jxl_cms.cc
compiler command, changing only the adapter translation unit and output path,
and link the untouched original CMS archive. A second observer includes the
complete pristine jxl_cms.cc with that same command and links its original
dependencies. Its public callbacks must agree with the archive context and both
ordinary/nonrecovering ASan+UBSan graphs. Retain exact statuses, external fields,
partial failures, aligned thread-view pointer relationships, all initialized
sample words and cleanup order. Private helpers retain their actual source
functions and source-defined prefix-only context/descriptor cases.

The source audit must verify live function addresses/object owners, unchanged
source/archive/object hashes, actual compiler/instrumentation commands, complete
retained requests and compiled-object arithmetic evidence. MatrixProduct retains
its float-zero initialization and three accumulation/write checkpoints, including
actual input/output aliasing. Native contraction evidence supplements exact
observations; it does not justify changing source flags or expected words.

Keep the twelve initial HLG NaN Rust panics alongside their successful actual
ordinary/ASan source observations. The selected NDEBUG source has no active HLG
JXL_DASSERT at those two sites. Removing only the corresponding Rust assertions
requires an explicit unchanged-arithmetic proof and complete inherited corpus
retention. Do not classify those successful source calls as undefined or remove
them from paired comparisons. Whole alternate-profile/ICC/HDR decoding and both
native platforms remain separate required acceptance gates.

Retain genuine callback probes with unknown inactive interface/descriptor
members, callback/data replacement, ICC-span replacement between calls, early
failure, nonzero success and active-field-only external outputs. Observe every
callback input byte and every initialized returned field, call order, descriptor
state and row output. Normalize only function/data identity tokens and buffer
alignment or relative pointer offsets. The eight initial callback row failures
remain mandatory: actual original ordinary/instrumented contexts agree, and
actual baseline-object instructions expose a rounded G product followed by R
and B FMAs in both skcms OOTF directions. An ARM checkpoint correction requires
the complete inherited corpus and all six final gates again; helper success
does not establish whole-file ICC/HDR admission or native two-platform closure.

The direct numeric corpus also retains all 854 initial CIExy NaN payload
differences. Both actual source graphs agree, and the actual private-object
FMUL operand order determines the target representation checkpoint. Compare
all returned float/double words exactly. Keep the original observations and
input hashes; pointer identity normalization never applies to numeric values.

Empty callback replacement vectors preserve their selected libc++ null data
pointers in both observers. Eight original callback cases expose the target's
initial null+zero slice abort; retain their full diagnostics and unchanged
requests/source observations. The corrected empty-copy path requires fresh
complete gates, not acceptance from a preceding runtime freeze.
The concatenation diagnostic additionally retains 432 defined calls with paired
quiet/signaling NaNs at every output-cell/product-term position. Both native ARM
original graphs agree on every word. All 24,324 existing concatenation inputs
and their captured outputs are retained byte for byte. The actual ARM source
object keeps A before B at the rounded middle product and both FMADD sites;
non-inlined Rust checkpoints retain that order in all three build profiles.
The paired inputs also run as a permanent exact differential group, and a
separate native ARM regression checks independently captured original words.
Native Linux operand selection and full native acceptance remain open until
both original graphs and all Rust profiles pass the same complete inventory.
