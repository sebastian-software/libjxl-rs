// Copyright (c) the JPEG XL Project Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license in LICENSE.
// Test protocol adapter for the unmodified pinned implementation.

#include <cstdint>
#include <cstring>
#include <iostream>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

#include "lib/jxl/base/span.h"
#include "lib/jxl/dec_bit_reader.h"
#include "lib/jxl/enc_aux_out.h"
#include "lib/jxl/enc_bit_writer.h"
#include "lib/jxl/field_encodings.h"
#include "lib/jxl/fields.h"
#include "lib/jxl/headers.h"
#include "lib/jxl/memory_manager_internal.h"

using jxl::BitReader;
using jxl::BitWriter;
using jxl::LayerType;
using jxl::Status;

int Code(Status status) { return static_cast<int>(status.code()); }

std::string Hex(jxl::Span<const uint8_t> bytes) {
  static constexpr char kDigits[] = "0123456789abcdef";
  if (bytes.empty()) return "-";
  std::string result;
  for (uint8_t byte : bytes) {
    result += kDigits[byte >> 4];
    result += kDigits[byte & 15];
  }
  return result;
}

std::vector<uint8_t> Unhex(const std::string& hex) {
  if (hex == "-") return {};
  if (hex.size() % 2 != 0) throw std::runtime_error("odd hex length");
  std::vector<uint8_t> bytes;
  for (size_t i = 0; i < hex.size(); i += 2) {
    bytes.push_back(static_cast<uint8_t>(std::stoul(hex.substr(i, 2), nullptr, 16)));
  }
  return bytes;
}

// Keep the address non-null even for an empty logical input, as upstream's
// BitReader constructor uses pointer arithmetic on the provided span.
struct Input {
  explicit Input(const std::string& hex) : data(Unhex(hex)), size(data.size()) {
    data.resize(size + 8);
  }
  jxl::Bytes bytes() const { return jxl::Bytes(data.data(), size); }
  std::vector<uint8_t> data;
  size_t size;
};

Status Pad(BitWriter& writer) {
  return writer.WithMaxBits(7, LayerType::Header, nullptr, [&] {
    writer.ZeroPadToByte();
    return true;
  });
}

Status Prefix(BitWriter& writer, size_t prefix) {
  if (prefix == 0) return true;
  return writer.WithMaxBits(prefix, LayerType::Header, nullptr, [&] {
    writer.Write(prefix, (1ULL << prefix) - 1);
    return true;
  });
}

template <typename Header>
void DimensionHeader(std::istringstream& in, JxlMemoryManager* mm) {
  size_t x, y, prefix;
  in >> x >> y >> prefix;
  Header header;
  Status set = header.Set(x, y);
  if (!set) {
    std::cout << "S " << Code(set);
    return;
  }
  size_t ext = 0, total = 0;
  Status can = jxl::Bundle::CanEncode(header, &ext, &total);
  if (!can) {
    std::cout << "C " << Code(can);
    return;
  }
  BitWriter writer(mm);
  if (!Prefix(writer, prefix)) throw std::runtime_error("prefix allocation");
  Status written = jxl::Bundle::Write(header, &writer, LayerType::Header, nullptr);
  size_t bits = writer.BitsWritten();
  if (!Pad(writer)) throw std::runtime_error("padding allocation");
  Header decoded;
  BitReader reader(writer.GetSpan());
  reader.SkipBits(prefix);
  Status read = jxl::Bundle::Read(&reader, &decoded);
  std::cout << ext << ' ' << total << ' ' << Code(written) << ' ' << bits << ' '
            << Hex(writer.GetSpan()) << ' ' << Code(read) << ' ' << decoded.xsize()
            << ' ' << decoded.ysize() << ' ' << reader.TotalBitsConsumed() << ' '
            << Code(reader.Close());
}

template <typename Header>
void ReadDimensionHeader(std::istringstream& in) {
  std::string hex;
  size_t offset;
  in >> hex >> offset;
  Input input(hex);
  BitReader reader(input.bytes());
  reader.SkipBits(offset);
  Header header;
  Status read = jxl::Bundle::Read(&reader, &header);
  std::cout << Code(read) << ' ' << header.xsize() << ' ' << header.ysize() << ' '
            << reader.TotalBitsConsumed() << ' ' << Code(reader.Close());
}

void Reader(std::istringstream& in) {
  std::string hex, op;
  in >> hex;
  Input input(hex);
  BitReader reader(input.bytes());
  while (in >> op) {
    uint64_t result = 0;
    if (op[0] == 'r') result = reader.ReadBits(std::stoull(op.substr(1)));
    else if (op[0] == 'p') result = reader.PeekBits(std::stoull(op.substr(1)));
    else if (op[0] == 'c') reader.Consume(std::stoull(op.substr(1)));
    else if (op[0] == 's') reader.SkipBits(std::stoull(op.substr(1)));
    else if (op == "f") reader.Refill();
    else if (op == "a") result = Code(reader.JumpToByteBoundary());
    else if (op == "b") result = Code(reader.AllReadsWithinBounds());
    else if (op == "m") reader.MarkUnhealthy();
    else throw std::runtime_error("unknown reader operation");
    std::cout << op << '=' << result << '@' << reader.TotalBitsConsumed() << ';';
  }
  in.clear();
  std::cout << "close=" << Code(reader.Close());
}

void Primitive(const std::string& kind, std::istringstream& in,
               JxlMemoryManager* mm) {
  uint64_t value;
  size_t prefix, width = 0;
  uint32_t raw[4] = {};
  if (kind == "u32") in >> raw[0] >> raw[1] >> raw[2] >> raw[3];
  if (kind == "bits") in >> width;
  in >> value >> prefix;
  jxl::U32Enc enc{jxl::U32Distr(raw[0]), jxl::U32Distr(raw[1]),
                  jxl::U32Distr(raw[2]), jxl::U32Distr(raw[3])};
  float fvalue = 0;
  uint32_t fbits = static_cast<uint32_t>(value);
  std::memcpy(&fvalue, &fbits, sizeof(fvalue));
  size_t encoded = 0;
  Status can = true;
  if (kind == "bits") can = jxl::BitsCoder::CanEncode(width, value, &encoded);
  else if (kind == "u32") can = jxl::U32Coder::CanEncode(enc, value, &encoded);
  else if (kind == "u64") can = jxl::U64Coder::CanEncode(value, &encoded);
  else can = jxl::F16Coder::CanEncode(fvalue, &encoded);
  BitWriter writer(mm);
  Status written = writer.WithMaxBits(96, LayerType::Header, nullptr, [&] {
    writer.Write(prefix, (1ULL << prefix) - 1);
    if (kind == "bits") return jxl::BitsCoder::Write(width, value, &writer);
    if (kind == "u32") return jxl::U32Coder::Write(enc, value, &writer);
    if (kind == "u64") return jxl::U64Coder::Write(value, &writer);
    return jxl::F16Coder::Write(fvalue, &writer);
  });
  const size_t bits = writer.BitsWritten();
  if (!Pad(writer)) throw std::runtime_error("padding allocation");
  std::cout << Code(can) << ' ' << encoded << ' ' << Code(written) << ' ' << bits
            << ' ' << Hex(writer.GetSpan());
  if (!written) return;
  BitReader reader(writer.GetSpan());
  reader.SkipBits(prefix);
  Status read = true;
  uint64_t decoded;
  if (kind == "bits") decoded = jxl::BitsCoder::Read(width, &reader);
  else if (kind == "u32") decoded = jxl::U32Coder::Read(enc, &reader);
  else if (kind == "u64") decoded = jxl::U64Coder::Read(&reader);
  else {
    float fdecoded = 123.0f;
    read = jxl::F16Coder::Read(&reader, &fdecoded);
    uint32_t bits32;
    std::memcpy(&bits32, &fdecoded, sizeof(bits32));
    decoded = bits32;
  }
  std::cout << ' ' << Code(read) << ' ' << decoded << ' '
            << reader.TotalBitsConsumed() << ' ' << Code(reader.Close());
}

void ReadHalf(std::istringstream& in) {
  uint32_t bits;
  in >> bits;
  uint8_t bytes[2] = {static_cast<uint8_t>(bits), static_cast<uint8_t>(bits >> 8)};
  BitReader reader(jxl::Bytes(bytes, 2));
  float value = 123.0f;
  Status read = jxl::F16Coder::Read(&reader, &value);
  uint32_t bits32;
  std::memcpy(&bits32, &value, sizeof(bits32));
  std::cout << Code(read) << ' ' << bits32 << ' ' << reader.TotalBitsConsumed()
            << ' ' << Code(reader.Close());
}

void Animation(std::istringstream& in, JxlMemoryManager* mm) {
  jxl::AnimationHeader header;
  size_t prefix;
  in >> header.tps_numerator >> header.tps_denominator >> header.num_loops
     >> header.have_timecodes >> prefix;
  size_t ext = 0, total = 0;
  Status can = jxl::Bundle::CanEncode(header, &ext, &total);
  if (!can) {
    std::cout << "C " << Code(can);
    return;
  }
  BitWriter writer(mm);
  if (!Prefix(writer, prefix)) throw std::runtime_error("prefix allocation");
  Status written = jxl::Bundle::Write(header, &writer, LayerType::Header, nullptr);
  size_t bits = writer.BitsWritten();
  if (!Pad(writer)) throw std::runtime_error("padding allocation");
  BitReader reader(writer.GetSpan());
  reader.SkipBits(prefix);
  jxl::AnimationHeader decoded;
  Status read = jxl::Bundle::Read(&reader, &decoded);
  std::cout << ext << ' ' << total << ' ' << Code(written) << ' ' << bits << ' '
            << Hex(writer.GetSpan()) << ' ' << Code(read) << ' '
            << decoded.tps_numerator << ' ' << decoded.tps_denominator << ' '
            << decoded.num_loops << ' ' << decoded.have_timecodes << ' '
            << reader.TotalBitsConsumed() << ' ' << Code(reader.Close());
}

void Nested(std::istringstream& in, JxlMemoryManager* mm) {
  size_t prefix, width;
  uint64_t value;
  bool fail, histogram;
  in >> prefix >> width >> value >> fail >> histogram;
  jxl::AuxOut aux;
  BitWriter writer(mm);
  Status inner = true;
  Status outer = writer.WithMaxBits(16, LayerType::Header, &aux, [&]() -> Status {
    writer.Write(prefix, (1ULL << prefix) - 1);
    inner = writer.WithMaxBits(64, LayerType::Ac, &aux, [&]() -> Status {
      writer.Write(width, value);
      return !fail;
    }, histogram);
    if (!inner) return inner;
    writer.Write(3, 5);
    return true;
  }, histogram);
  size_t bits = writer.BitsWritten();
  if (!Pad(writer) || !writer.Shrink()) throw std::runtime_error("writer storage");
  std::cout << Code(outer) << ' ' << Code(inner) << ' ' << bits << ' '
            << aux.layer(LayerType::Header).total_bits << ' '
            << aux.layer(LayerType::Header).histogram_bits << ' '
            << aux.layer(LayerType::Ac).total_bits << ' '
            << aux.layer(LayerType::Ac).histogram_bits << ' ' << aux.TotalBits()
            << ' ' << Hex(writer.GetSpan());
}

// Fixture definitions from upstream lib/jxl/fields_test.cc. The visitors and
// all encoding/decoding operations below are the original upstream functions.
struct OldBundle : public jxl::Fields {
  OldBundle() { jxl::Bundle::Init(this); }
  JXL_FIELDS_NAME(OldBundle)
  Status VisitFields(jxl::Visitor* v) override {
    JXL_QUIET_RETURN_IF_ERROR(v->U32(jxl::Val(1), jxl::Bits(2), jxl::Bits(3), jxl::Bits(4), 1, &old_small));
    JXL_QUIET_RETURN_IF_ERROR(v->F16(1.125f, &old_f));
    JXL_QUIET_RETURN_IF_ERROR(v->U32(jxl::Bits(7), jxl::Bits(12), jxl::Bits(16), jxl::Bits(32), 0, &old_large));
    JXL_QUIET_RETURN_IF_ERROR(v->BeginExtensions(&extensions));
    return v->EndExtensions();
  }
  uint32_t old_small;
  float old_f;
  uint32_t old_large;
  uint64_t extensions;
};

struct NewBundle : public jxl::Fields {
  NewBundle() { jxl::Bundle::Init(this); }
  JXL_FIELDS_NAME(NewBundle)
  Status VisitFields(jxl::Visitor* v) override {
    JXL_QUIET_RETURN_IF_ERROR(v->U32(jxl::Val(1), jxl::Bits(2), jxl::Bits(3), jxl::Bits(4), 1, &old_small));
    JXL_QUIET_RETURN_IF_ERROR(v->F16(1.125f, &old_f));
    JXL_QUIET_RETURN_IF_ERROR(v->U32(jxl::Bits(7), jxl::Bits(12), jxl::Bits(16), jxl::Bits(32), 0, &old_large));
    JXL_QUIET_RETURN_IF_ERROR(v->BeginExtensions(&extensions));
    if (v->Conditional((extensions & 1) != 0)) {
      JXL_QUIET_RETURN_IF_ERROR(v->U32(jxl::Val(2), jxl::Bits(2), jxl::Bits(3), jxl::Bits(4), 2, &new_small));
      JXL_QUIET_RETURN_IF_ERROR(v->F16(-2.0f, &new_f));
    }
    if (v->Conditional((extensions & 2) != 0)) {
      JXL_QUIET_RETURN_IF_ERROR(v->U32(jxl::Bits(9), jxl::Bits(12), jxl::Bits(16), jxl::Bits(32), 0, &new_large));
    }
    return v->EndExtensions();
  }
  uint32_t old_small;
  float old_f;
  uint32_t old_large;
  uint64_t extensions;
  uint32_t new_small = 2;
  float new_f = -2.0f;
  uint32_t new_large = 0;
};

void ExtraFields(const OldBundle&) { std::cout << " - - -"; }
void ExtraFields(const NewBundle& fields) {
  uint32_t bits;
  std::memcpy(&bits, &fields.new_f, sizeof(bits));
  std::cout << ' ' << fields.new_small << ' ' << bits << ' ' << fields.new_large;
}

template <typename B>
void ReadBundle(jxl::Bytes bytes, size_t prefix) {
  B decoded;
  BitReader reader(bytes);
  reader.SkipBits(prefix);
  Status read = jxl::Bundle::Read(&reader, &decoded);
  uint32_t bits;
  std::memcpy(&bits, &decoded.old_f, sizeof(bits));
  std::cout << ' ' << Code(read) << ' ' << decoded.old_small << ' ' << bits << ' '
            << decoded.old_large << ' ' << decoded.extensions;
  ExtraFields(decoded);
  size_t consumed = reader.TotalBitsConsumed();
  uint64_t sentinel = reader.ReadBits(20);
  std::cout << ' ' << consumed << ' ' << sentinel << ' ' << Code(reader.Close());

  B checked;
  BitReader probe(bytes);
  probe.SkipBits(prefix);
  bool can_read = jxl::Bundle::CanRead(&probe, &checked);
  std::cout << ' ' << can_read << ' ' << probe.TotalBitsConsumed() << ' '
            << Code(probe.Close());
}

void BundleCase(std::istringstream& in, JxlMemoryManager* mm) {
  NewBundle fields;
  uint32_t old_bits, new_bits;
  bool decode_new;
  size_t prefix, limit;
  in >> fields.extensions >> fields.old_small >> old_bits >> fields.old_large
     >> fields.new_small >> new_bits >> fields.new_large >> decode_new >> prefix >> limit;
  std::memcpy(&fields.old_f, &old_bits, sizeof(old_bits));
  std::memcpy(&fields.new_f, &new_bits, sizeof(new_bits));
  size_t ext = 0, total = 0;
  Status can = jxl::Bundle::CanEncode(fields, &ext, &total);
  if (!can) { std::cout << "C " << Code(can); return; }
  BitWriter writer(mm);
  if (!Prefix(writer, prefix)) throw std::runtime_error("prefix allocation");
  Status written = jxl::Bundle::Write(fields, &writer, LayerType::Header, nullptr);
  const size_t bits = writer.BitsWritten();
  if (!writer.WithMaxBits(32, LayerType::Header, nullptr, [&] {
    writer.Write(20, 0xA55A);
    writer.ZeroPadToByte();
    return true;
  })) throw std::runtime_error("sentinel allocation");
  std::cout << ext << ' ' << total << ' ' << Code(written) << ' ' << bits << ' '
            << Hex(writer.GetSpan());
  const auto span = writer.GetSpan();
  const jxl::Bytes input(span.data(), std::min(limit, span.size()));
  if (decode_new) ReadBundle<NewBundle>(input, prefix);
  else ReadBundle<OldBundle>(input, prefix);
}

struct DefaultedBundle : public jxl::Fields {
  DefaultedBundle() { jxl::Bundle::Init(this); }
  JXL_FIELDS_NAME(DefaultedBundle)
  Status VisitFields(jxl::Visitor* v) override {
    if (v->AllDefault(*this, &all_default)) {
      v->SetDefault(this);
      return true;
    }
    JXL_QUIET_RETURN_IF_ERROR(v->Bits(4, 7, &value));
    return v->VisitNested(&size);
  }
  bool all_default;
  uint32_t value;
  jxl::SizeHeader size;
};

void DefaultCase(std::istringstream& in, JxlMemoryManager* mm) {
  DefaultedBundle fields;
  size_t x, y, prefix, limit;
  in >> fields.value >> x >> y >> prefix >> limit;
  if ((x != 0 || y != 0) && !fields.size.Set(x, y)) {
    std::cout << "S 1";
    return;
  }
  size_t ext = 0, total = 0;
  Status can = jxl::Bundle::CanEncode(fields, &ext, &total);
  if (!can) { std::cout << "C " << Code(can); return; }
  BitWriter writer(mm);
  if (!Prefix(writer, prefix)) throw std::runtime_error("prefix allocation");
  Status written = jxl::Bundle::Write(fields, &writer, LayerType::Header, nullptr);
  size_t bits = writer.BitsWritten();
  if (!Pad(writer)) throw std::runtime_error("padding allocation");
  auto bytes = writer.GetSpan();
  DefaultedBundle decoded;
  BitReader reader(jxl::Bytes(bytes.data(), std::min(limit, bytes.size())));
  reader.SkipBits(prefix);
  Status read = jxl::Bundle::Read(&reader, &decoded);
  std::cout << ext << ' ' << total << ' ' << Code(written) << ' ' << bits << ' '
            << Hex(bytes) << ' ' << Code(read) << ' ' << fields.all_default << ' '
            << decoded.all_default << ' ' << decoded.value << ' '
            << decoded.size.xsize() << ' ' << decoded.size.ysize() << ' '
            << reader.TotalBitsConsumed() << ' ' << Code(reader.Close());
}

void AppendCase(std::istringstream& in, JxlMemoryManager* mm) {
  size_t prefix, width;
  uint64_t value;
  std::string suffix;
  in >> prefix >> width >> value >> suffix;
  BitWriter writer(mm), other(mm);
  if (!Prefix(writer, prefix)) throw std::runtime_error("prefix allocation");
  if (!other.WithMaxBits(64, LayerType::Header, nullptr, [&] {
    other.Write(width, value);
    return true;
  })) throw std::runtime_error("source allocation");
  Status unaligned = writer.AppendUnaligned(other);
  if (!Pad(writer)) throw std::runtime_error("padding allocation");
  Input bytes(suffix);
  Status aligned = writer.AppendByteAligned(bytes.bytes());
  std::vector<std::unique_ptr<BitWriter>> parts;
  for (size_t n : {0, 1, 2}) {
    parts.emplace_back(new BitWriter(mm));
    if (!parts.back()->WithMaxBits(24, LayerType::Header, nullptr, [&] {
      for (size_t i = 0; i < n; ++i) parts.back()->Write(8, 0xEF - i);
      return true;
    })) throw std::runtime_error("part allocation");
  }
  Status group = writer.AppendByteAligned(parts);
  if (!writer.WithMaxBits(16, LayerType::Header, nullptr, [&] {
    writer.Write(13, 0x1ABC);
    writer.ZeroPadToByte();
    return true;
  }) || !writer.Shrink()) throw std::runtime_error("final allocation");
  std::cout << Code(unaligned) << ' ' << Code(aligned) << ' ' << Code(group) << ' '
            << writer.BitsWritten() << ' ' << Hex(writer.GetSpan());
}

#include "entropy.inc"
#include "huffman.inc"
#include "ans.inc"
#include "cluster.inc"
#include "lz77.inc"
#include "enc_entropy.inc"
#include "cms.inc"
#include "metadata.inc"
#include "frame.inc"
#include "toc.inc"
#include "coeff_orders.inc"
#include "image.inc"
#include "rct.inc"
#include "predict.inc"
#include "ma.inc"
#include "transforms.inc"
#include "modular_decode.inc"
#include "learn.inc"
#include "enc_frame.inc"
#include "aux_stats.inc"
#include "frame_output.inc"
#include "external_image.inc"
#include "quant_dc.inc"

bool DctCommand(const std::string&, std::istringstream&);
bool SampleConversionCommand(const std::string& command, std::istringstream& in);
bool ModularFrameCommand(const std::string& command, std::istringstream& in, JxlMemoryManager* mm);
bool ModularFrameStateCommand(const std::string& command, std::istringstream& in, JxlMemoryManager* mm);
bool QuantAcCommand(const std::string&,std::istringstream&,JxlMemoryManager*);
bool QuantizerCommand(const std::string&,std::istringstream&,JxlMemoryManager*);
bool QuantStorageCommand(const std::string& command,std::istringstream& in);
bool FrameBuffersCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool SharedPrimitivesCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool ModularFrameDecoderCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool RenderPipelineCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool ModularGroupsCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool SpotCommand(const std::string&,std::istringstream&,JxlMemoryManager*);
bool EPFCommand(const std::string&,std::istringstream&,JxlMemoryManager*);
bool GaborishStageCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool UpsamplingCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool OutputEncodingCommand(const std::string& command,std::istringstream& in);
bool DecoderStateCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool CmsStageCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool XybStageCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool ToneMappingCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool DecodeFrameCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool FloatOutputCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool UintOutputCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool ACGroupSymbolsCommand(const std::string&,std::istringstream&,JxlMemoryManager*);
bool BlockContextCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);
bool WholeImageCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm);

bool VarDctImageCommand(const std::string&,std::istringstream&,JxlMemoryManager*);

bool VarDCTDCCommand(const std::string&,std::istringstream&,JxlMemoryManager*);

bool GroupScheduleCommand(const std::string&,std::istringstream&,JxlMemoryManager*);

bool DequantCommand(const std::string&,std::istringstream&,JxlMemoryManager*);

bool GroupRenderCommand(const std::string&,std::istringstream&,JxlMemoryManager*);

int main() {
  try {
    JxlMemoryManager mm;
    if (!jxl::MemoryManagerInit(&mm, nullptr)) return 2;
    std::string line;
    while (std::getline(std::cin, line)) {
      std::istringstream in(line);
      std::string command;
      in >> command;
      if (command == "reader") Reader(in);
      else if (command == "size") DimensionHeader<jxl::SizeHeader>(in, &mm);
      else if (command == "preview") DimensionHeader<jxl::PreviewHeader>(in, &mm);
      else if (command == "read-size") ReadDimensionHeader<jxl::SizeHeader>(in);
      else if (command == "read-preview") ReadDimensionHeader<jxl::PreviewHeader>(in);
      else if (command == "f16-read") ReadHalf(in);
      else if (command == "animation") Animation(in, &mm);
      else if (command == "nested") Nested(in, &mm);
      else if (command == "bundle") BundleCase(in, &mm);
      else if (command == "default") DefaultCase(in, &mm);
      else if (command == "append") AppendCase(in, &mm);
      else if (HuffmanCommand(command, in, &mm)) {}
      else if (ANSCommand(command, in, &mm)) {}
      else if (ClusterCommand(command, in)) {}
      else if (LZ77Command(command, in)) {}
      else if (EncodeEntropyCommand(command, in, &mm)) {}
      else if (CmsCommand(command, in)) {}
      else if (MetadataCommand(command, in, &mm)) {}
      else if (FrameCommand(command, in, &mm)) {}
      else if (TocCommand(command, in, &mm)) {}
      else if (CoeffOrdersCommand(command, in, &mm)) {}
      else if (ImageCommand(command, in, &mm)) {}
      else if (RctCommand(command, in, &mm)) {}
      else if (PredictCommand(command, in, &mm)) {}
      else if (MaCommand(command, in, &mm)) {}
      else if (TransformsCommand(command, in, &mm)) {}
      else if (ModularDecodeCommand(command, in, &mm)) {}
      else if (LearnCommand(command, in, &mm)) {}
      else if (EncFrameCommand(command, in, &mm)) {}
      else if (AuxStatsCommand(command, in)) {}
      else if (FrameOutputCommand(command, in, &mm)) {}
      else if (ExternalImageCommand(command, in, &mm)) {}
      else if (QuantDcCommand(command, in, &mm)) {}
      else if (QuantStorageCommand(command, in)) {}
      else if (DctCommand(command, in)) {}
      else if (QuantAcCommand(command, in, &mm)) {}
      else if (QuantizerCommand(command, in, &mm)) {}
      else if (SampleConversionCommand(command, in)) {}
      else if (ModularFrameCommand(command, in, &mm)) {}
      else if (ModularFrameStateCommand(command, in, &mm)) {}
      else if (FrameBuffersCommand(command, in, &mm)) {}
      else if (SharedPrimitivesCommand(command, in, &mm)) {}
      else if (ModularFrameDecoderCommand(command, in, &mm)) {}
      else if (RenderPipelineCommand(command, in, &mm)) {}
      else if (ModularGroupsCommand(command, in, &mm)) {}
      else if (SpotCommand(command, in, &mm)) {}
      else if (EPFCommand(command, in, &mm)) {}
      else if (GaborishStageCommand(command, in, &mm)) {}
      else if (UpsamplingCommand(command, in, &mm)) {}
      else if (OutputEncodingCommand(command, in)) {}
      else if (DecoderStateCommand(command, in, &mm)) {}
      else if (CmsStageCommand(command, in, &mm)) {}
      else if (XybStageCommand(command, in, &mm)) {}
      else if (ToneMappingCommand(command, in, &mm)) {}
      else if (DecodeFrameCommand(command, in, &mm)) {}
      else if (UintOutputCommand(command, in, &mm)) {}
      else if (FloatOutputCommand(command, in, &mm)) {}
      else if (WholeImageCommand(command, in, &mm)) {}
      else if (VarDctImageCommand(command, in, &mm)) {}
      else if (GroupRenderCommand(command, in, &mm)) {}
      else if (DequantCommand(command, in, &mm)) {}
      else if (GroupScheduleCommand(command, in, &mm)) {}
      else if (VarDCTDCCommand(command, in, &mm)) {}
      else if (ACGroupSymbolsCommand(command, in, &mm)) {}
      else if (BlockContextCommand(command, in, &mm)) {}
      else if (EntropyCommand(command, in, &mm)) {}
      else if (command == "bits" || command == "u32" || command == "u64" || command == "f32")
        Primitive(command, in, &mm);
      else throw std::runtime_error("unknown command");
      if (in.fail()) throw std::runtime_error("invalid request");
      std::cout << '\n';
    }
    return 0;
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 2;
  }
}
