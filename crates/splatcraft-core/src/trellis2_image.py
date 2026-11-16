"""One photo to a TRELLIS.2 textured GLB. Written into a job directory and run by splatcraft."""

import argparse
import os
import sys

os.environ.setdefault("OPENCV_IO_ENABLE_OPENEXR", "1")
os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")


def main() -> int:
    parser = argparse.ArgumentParser(description="Single photo to a TRELLIS.2 GLB")
    parser.add_argument("--image", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument(
        "--pipeline",
        default="512",
        choices=("512", "1024_cascade", "1536_cascade"),
    )
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()
    try:
        from PIL import Image
        import o_voxel
        from trellis2.pipelines import Trellis2ImageTo3DPipeline
    except ImportError as exc:
        sys.stderr.write(
            "TRELLIS.2 is not importable. The official install is Linux, an NVIDIA "
            "GPU with 24GB, CUDA 12.4, and the weights microsoft/TRELLIS.2-4B. "
            f"{exc}\n"
        )
        return 2
    image = Image.open(args.image)
    image.load()
    pipeline = Trellis2ImageTo3DPipeline.from_pretrained("microsoft/TRELLIS.2-4B")
    pipeline.cuda()
    mesh = pipeline.run(image, seed=args.seed, pipeline_type=args.pipeline)[0]
    mesh.simplify(16777216)
    parent = os.path.dirname(os.path.abspath(args.output))
    if parent:
        os.makedirs(parent, exist_ok=True)
    glb = o_voxel.postprocess.to_glb(
        vertices=mesh.vertices,
        faces=mesh.faces,
        attr_volume=mesh.attrs,
        coords=mesh.coords,
        attr_layout=mesh.layout,
        voxel_size=mesh.voxel_size,
        aabb=[[-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]],
        decimation_target=1000000,
        texture_size=4096,
        remesh=True,
        remesh_band=1,
        remesh_project=0,
        verbose=True,
    )
    glb.export(args.output, extension_webp=True)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
