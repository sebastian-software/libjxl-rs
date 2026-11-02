// Copyright (c) the JPEG XL Project Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license in LICENSE.
// Boundary observer; the included pristine file supplies its actual factory.
#include <array>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>
#include "lib/jxl/frame_dimensions.h"
#include "lib/jxl/image.h"
#include "lib/jxl/render_pipeline/simple_render_pipeline.h"
#include "lib/jxl/render_pipeline/stage_write.h"
#include "lib/jxl/render_pipeline/stage_spot.cc"
float RPValue(size_t,size_t,size_t,size_t,unsigned);
void RPPipeline(jxl::SimpleRenderPipeline&);
void MGOutput(const jxl::Image3F&);
namespace {
template<class T,class U> T Word(U value){T out;static_assert(sizeof(T)==sizeof(U));std::memcpy(&out,&value,sizeof(out));return out;}
std::array<float,4> Color(std::istringstream& in){std::array<float,4> a;for(auto&v:a){uint32_t b;in>>b;v=Word<float>(b);}return a;}
void PrintColor(const std::array<float,4>&a){for(float v:a)std::cout<<' '<<Word<uint32_t>(v);}
}
bool SpotCommand(const std::string& command,std::istringstream& in,JxlMemoryManager* mm){
 if(command.rfind("spot-",0)!=0)return false;
 if(command=="spot-simple"){
  size_t w,h,count,threads;unsigned mode;bool reverse;in>>w>>h>>count>>threads>>mode>>reverse;
  std::vector<size_t> offsets(count);std::vector<std::array<float,4>> colors(count),changed(count);
  for(size_t i=0;i<count;i++){in>>offsets[i];colors[i]=Color(in);changed[i]=Color(in);}
  jxl::FrameDimensions dim;dim.Set(w,h,0,0,0,true,1);jxl::Image3F image;
  {
   jxl::RenderPipeline::Builder builder(mm,7);builder.UseSimpleImplementation();
   for(size_t i=0;i<count;i++){auto s=builder.AddStage(jxl::GetSpotColorStage(offsets[i],colors[i].data()));std::cout<<" B "<<int(s.code());if(!s)return true;}
   // Change the actual external arrays after construction, before row execution.
   for(size_t i=0;i<count;i++)colors[i]=changed[i];
   if(!builder.AddStage(jxl::GetWriteToImage3FStage(mm,&image)))throw std::runtime_error("spot writer");
   auto made=std::move(builder).Finalize(dim);if(!made.ok())throw std::runtime_error("spot pipeline");
   auto base=std::move(made).value_();auto&pipeline=*static_cast<jxl::SimpleRenderPipeline*>(base.get());
   auto s=pipeline.PrepareForThreads(threads,true);std::cout<<" T "<<int(s.code());
   if(s)for(size_t i=0;i<dim.num_groups;i++){
    size_t g=reverse?dim.num_groups-1-i:i;auto input=pipeline.GetInputBuffers(g,i%threads);
    for(size_t c=0;c<7;c++){auto[plane,r]=input.GetBuffer(c);for(size_t y=0;y<r.ysize();y++)for(size_t x=0;x<r.xsize();x++)r.Row(plane,y)[x]=RPValue(c,r.x0()+x-jxl::kRenderPipelineXOffset,r.y0()+y-jxl::kRenderPipelineXOffset,0,mode);}
    s=input.Done();std::cout<<" D "<<int(s.code());if(!s)break;
   }
   RPPipeline(pipeline);
  }
  MGOutput(image);for(auto&a:colors)PrintColor(a);return true;
 }
 size_t offset;unsigned owner;in>>offset>>owner;auto color=Color(in);auto stage=jxl::GetSpotColorStage(offset,color.data());
 if(command=="spot-query"){
  auto s=stage->settings_;std::cout<<stage->GetName()<<' '<<s.border_x<<' '<<s.border_y<<' '<<s.shift_x<<' '<<s.shift_y<<' '<<int(stage->IsInitialized().code())<<' '<<int(stage->PrepareForThreads(3).code());
  size_t count;in>>count;for(size_t i=0;i<count;i++){size_t c;in>>c;std::cout<<' '<<int(stage->GetChannelMode(c));}PrintColor(color);return true;
 }
 if(command=="spot-guard"){
  size_t l,r;in>>l>>r;jxl::RenderPipelineStage::RowInfo empty;auto s=stage->ProcessRow(empty,empty,l,r,23,91,73,17);std::cout<<int(s.code());PrintColor(color);return true;
 }
 if(command=="spot-sample"){
  uint32_t p,s;in>>p>>s;std::vector<jxl::ImageF> planes;for(size_t c=0;c<4;c++){auto made=jxl::ImageF::Create(mm,jxl::kRenderPipelineXOffset+1,1);if(!made.ok())throw std::runtime_error("spot sample");planes.push_back(std::move(made).value_());planes.back().Row(0)[jxl::kRenderPipelineXOffset]=Word<float>(c==3?s:p);}
  jxl::RenderPipelineStage::RowInfo rows(4);for(size_t c=0;c<4;c++)rows[c].push_back(planes[c].Row(0));
  auto status=stage->ProcessRow(rows,{},0,0,1,0,0,0);std::cout<<int(status.code());for(auto&p:planes)std::cout<<' '<<Word<uint32_t>(p.Row(0)[jxl::kRenderPipelineXOffset]);PrintColor(color);return true;
 }
 size_t width,left,right,origin,passes;unsigned mode;in>>width>>left>>right>>origin>>passes>>mode;
 const size_t n=7,len=origin+jxl::kRenderPipelineXOffset+width+9;
 std::vector<jxl::ImageF> planes;for(size_t c=0;c<n;c++){auto made=jxl::ImageF::Create(mm,len,3);if(!made.ok())throw std::runtime_error("spot row");planes.push_back(std::move(made).value_());for(size_t y=0;y<3;y++)for(size_t x=0;x<len;x++)planes[c].Row(y)[x]=RPValue(c,x,y,0,mode);}
 jxl::RenderPipelineStage::RowInfo rows(n);for(size_t c=0;c<n;c++)rows[c].push_back(planes[c].Row(1)+origin);
 for(size_t pass=0;pass<passes;pass++){auto changed=Color(in);if(owner)color=changed;auto status=stage->ProcessRow(rows,{},left,right,width,97,131,19);std::cout<<' '<<int(status.code());for(auto&p:planes)for(size_t y=0;y<3;y++)for(size_t x=0;x<len;x++)std::cout<<' '<<Word<uint32_t>(p.Row(y)[x]);PrintColor(color);}
 return true;
}
