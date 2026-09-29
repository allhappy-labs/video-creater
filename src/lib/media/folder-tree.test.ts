import { describe, expect, it } from "vitest";
import type { MediaFolder } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import {
  flattenMediaFolderNodes,
  mediaFolderIdForName,
  mediaFolderPathLabel,
  mediaFolderSlug,
  mediaFolderTree,
  type MediaFolderNode,
} from "@/lib/media/folder-tree";

const nestedFolders: MediaFolder[] = [
  { id: "root", name: "Root", parentId: null },
  { id: "child", name: "Child", parentId: "root" },
  { id: "grandchild", name: "Grandchild", parentId: "child" },
  { id: "sibling", name: "Sibling", parentId: "root" },
  { id: "second-root", name: "Second root", parentId: null },
];
const orphanFolders: MediaFolder[] = [
  { id: "root", name: "Root", parentId: null },
  { id: "orphan", name: "Orphan", parentId: "missing-parent" },
  { id: "orphan-child", name: "Orphan child", parentId: "orphan" },
];
const cycleFolders: MediaFolder[] = [
  { id: "a", name: "A", parentId: "b" },
  { id: "b", name: "B", parentId: "a" },
  { id: "c", name: "C", parentId: "a" },
  { id: "self", name: "Self", parentId: "self" },
];

function summarizeNode(node: MediaFolderNode): unknown {
  return {
    id: node.folder.id,
    label: node.label,
    depth: node.depth,
    children: node.children.map(summarizeNode),
  };
}

function flatSummary(nodes: MediaFolderNode[]) {
  return flattenMediaFolderNodes(nodes).map((node) => `${node.depth}:${node.folder.id}:${node.label}`);
}

describe("media folder tree characterization", () => {
  it("builds nested folder trees", () => {
    const tree = mediaFolderTree(nestedFolders);
    expect(tree.map(summarizeNode)).toMatchInlineSnapshot(`
      [
        {
          "children": [
            {
              "children": [
                {
                  "children": [],
                  "depth": 2,
                  "id": "grandchild",
                  "label": "Root / Child / Grandchild",
                },
              ],
              "depth": 1,
              "id": "child",
              "label": "Root / Child",
            },
            {
              "children": [],
              "depth": 1,
              "id": "sibling",
              "label": "Root / Sibling",
            },
          ],
          "depth": 0,
          "id": "root",
          "label": "Root",
        },
        {
          "children": [],
          "depth": 0,
          "id": "second-root",
          "label": "Second root",
        },
      ]
    `);
    expect(flatSummary(tree)).toMatchInlineSnapshot(`
      [
        "0:root:Root",
        "1:child:Root / Child",
        "2:grandchild:Root / Child / Grandchild",
        "1:sibling:Root / Sibling",
        "0:second-root:Second root",
      ]
    `);
    expect(mediaFolderTree([]).map(summarizeNode)).toMatchInlineSnapshot(`[]`);
    expect(flatSummary(mediaFolderTree(fixtureProject().mediaFolders ?? []))).toMatchInlineSnapshot(`
      [
        "0:folder-source:Source footage",
        "0:folder-generated:Generated selects",
        "0:folder-audio:Audio",
      ]
    `);
  });

  it("promotes orphans and cycles to roots", () => {
    expect(mediaFolderTree(orphanFolders).map(summarizeNode)).toMatchInlineSnapshot(`
      [
        {
          "children": [],
          "depth": 0,
          "id": "root",
          "label": "Root",
        },
        {
          "children": [
            {
              "children": [],
              "depth": 1,
              "id": "orphan-child",
              "label": "Orphan / Orphan child",
            },
          ],
          "depth": 0,
          "id": "orphan",
          "label": "Orphan",
        },
      ]
    `);
    expect(flatSummary(mediaFolderTree(orphanFolders))).toMatchInlineSnapshot(`
      [
        "0:root:Root",
        "0:orphan:Orphan",
        "1:orphan-child:Orphan / Orphan child",
      ]
    `);
    expect(mediaFolderTree(cycleFolders).map(summarizeNode)).toMatchInlineSnapshot(`
      [
        {
          "children": [],
          "depth": 0,
          "id": "a",
          "label": "A",
        },
        {
          "children": [],
          "depth": 0,
          "id": "b",
          "label": "B",
        },
        {
          "children": [],
          "depth": 0,
          "id": "c",
          "label": "C",
        },
        {
          "children": [],
          "depth": 0,
          "id": "self",
          "label": "Self",
        },
      ]
    `);
    expect(flatSummary(mediaFolderTree(cycleFolders))).toMatchInlineSnapshot(`
      [
        "0:a:A",
        "0:b:B",
        "0:c:C",
        "0:self:Self",
      ]
    `);
  });

  it("labels folder paths", () => {
    const cases: Array<[MediaFolder[], string | null | undefined]> = [
      ...nestedFolders.map((folder): [MediaFolder[], string] => [nestedFolders, folder.id]),
      ...orphanFolders.map((folder): [MediaFolder[], string] => [orphanFolders, folder.id]),
      ...cycleFolders.map((folder): [MediaFolder[], string] => [cycleFolders, folder.id]),
      [nestedFolders, null],
      [nestedFolders, undefined],
      [nestedFolders, ""],
      [nestedFolders, "missing"],
      [[], "root"],
    ];
    const labels = cases.map(([folders, folderId]) => mediaFolderPathLabel(folders, folderId));
    expect(labels).toMatchInlineSnapshot(`
      [
        "Root",
        "Root / Child",
        "Root / Child / Grandchild",
        "Root / Sibling",
        "Second root",
        "Root",
        "Orphan",
        "Orphan / Orphan child",
        "B / A",
        "A / B",
        "B / A / C",
        "Self",
        null,
        null,
        null,
        null,
        null,
      ]
    `);
  });

  it("slugs folder names", () => {
    expect(
      [
        "Source footage",
        "  B-Roll!!  ",
        "Voice / Over (final)",
        "émoji 🎬 clips",
        "日本語",
        "---",
        "",
        "Already-slugged_name 2",
      ].map((name) => mediaFolderSlug(name)),
    ).toMatchInlineSnapshot(`
      [
        "source-footage",
        "b-roll",
        "voice-over-final",
        "moji-clips",
        "folder",
        "folder",
        "folder",
        "already-slugged-name-2",
      ]
    `);
  });

  it("allocates unique folder ids for duplicate names", () => {
    const folders: MediaFolder[] = [
      { id: "folder-b-roll", name: "B-Roll", parentId: null },
      { id: "folder-b-roll-2", name: "B Roll", parentId: null },
      { id: "folder-b-roll-4", name: "B roll", parentId: null },
      { id: "folder-folder", name: "???", parentId: null },
    ];
    expect(
      [
        mediaFolderIdForName("B-Roll", folders),
        mediaFolderIdForName("b roll", folders),
        mediaFolderIdForName("New folder", folders),
        mediaFolderIdForName("日本語", folders),
        mediaFolderIdForName("!!!", []),
        mediaFolderIdForName("B-Roll", []),
      ],
    ).toMatchInlineSnapshot(`
      [
        "folder-b-roll-3",
        "folder-b-roll-3",
        "folder-new-folder",
        "folder-folder-2",
        "folder-folder",
        "folder-b-roll",
      ]
    `);
  });
});
