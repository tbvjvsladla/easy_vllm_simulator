"""Header-only aggregation regression; synthetic metadata, no weights or GPU execution."""
import json
from pathlib import Path
import struct
import tempfile
import unittest

import roofline


class ActiveTrafficAggregation(unittest.TestCase):
    def aggregate(self, with_engram):
        tensors = {
            "model.layers.0.self_attn.weight": 1000,
            "model.layers.0.mlp.shared_experts.weight": 400,
            "model.embed.weight": 200,
            "model.layers.0.engram.embed_aux.weight": 50,
            "model.layers.0.mlp.experts.0.weight": 1600,
            "model.layers.0.mlp.experts.0.scale": 400,
            "model.layers.0.ple.ple_embedding.ngram_embedding.shard_0.weight": 4096,
            "model.layers.1.ngram_embedding.weight": 1024,
            "model.mtp.layers.0.weight": 3000,
            "model.mtp.layers.0.engram.embed.weight": 50000,
        }
        if with_engram:
            tensors.update({"model.layers.1.engram.embed.weight": 100000,
                            "model.layers.1.engram.embed.scale": 20000})
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            weight_map = {}
            for shard in range(2):
                name = f"part-{shard}.safetensors"
                header, offset = {}, 0
                for index, (key, size) in enumerate(tensors.items()):
                    if index % 2 != shard:
                        continue
                    header[key] = {"dtype": "U8", "shape": [size],
                                   "data_offsets": [offset, offset + size]}
                    weight_map[key] = name
                    offset += size
                data = json.dumps(header).encode()
                (root / name).write_bytes(struct.pack("<Q", len(data)) + data)
            (root / "model.safetensors.index.json").write_text(json.dumps({"weight_map": weight_map}))
            kinds = {}
            result = roofline._active_bytes_moe(str(root), 8, 2, kinds)
        return result, kinds

    def test_sparse_tables_leave_dense_and_routed_traffic_unchanged(self):
        before, prior_kinds = self.aggregate(False)
        after, kinds = self.aggregate(True)
        self.assertEqual(before[:5], (2150, 1650, 2000, 2, 8))
        self.assertEqual(after[:5], before[:5])
        self.assertEqual(before[5], 5120)
        self.assertEqual(after[5] - before[5], 120000)
        self.assertEqual(prior_kinds, {"ple": 5120})
        self.assertEqual(kinds, {"ple": 5120, "engram": 120000})


if __name__ == "__main__":
    unittest.main()
