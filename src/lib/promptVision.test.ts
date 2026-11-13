import {describe,it,expect} from "vitest";
import {promptVisionErrorKey, resolveVisualSourcePaths} from "./promptVision";
describe("prompt vision boundaries",()=>{
 it("never silently chooses a subset above three and removes duplicate reference paths",()=>{
  expect(resolveVisualSourcePaths(["a","b","c","d"],null)).toEqual([]);
  expect(resolveVisualSourcePaths(["a","a","b"],null)).toEqual(["a","b"]);
  expect(resolveVisualSourcePaths(["a","b"],["b","missing","b"])).toEqual(["b"]);
 });
 it("maps only known safe errors and never returns provider error details",()=>{
  expect(promptVisionErrorKey({kind:"Validation",detail:{message:"prompt_vision_image_unavailable"}})).toBe("promptVision.imageUnavailable");
  expect(promptVisionErrorKey("prompt_vision_multimodal_required")).toBe("promptVision.multimodalRequired");
  expect(promptVisionErrorKey(new Error("https://provider/key=secret sk-123"))).toBe("promptVision.analysisFailed");
 });
});
