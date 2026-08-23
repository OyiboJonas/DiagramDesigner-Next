use serde::Serialize;

use crate::{
    container::LegacyContainer,
    object::{LegacyCurveBase, LegacyObject},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct LegacyObjectId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedTarget {
    pub object_id: LegacyObjectId,
    pub link_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEndpoint {
    pub source_object_id: LegacyObjectId,
    /// Legacy endpoint number: 1=start, 2=end.
    pub endpoint: u8,
    pub raw_object_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_link_index: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ResolvedTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReferenceIssue {
    InvalidObjectIndex {
        source_object_id: LegacyObjectId,
        endpoint: u8,
        object_index: i32,
        owner_object_count: usize,
    },
    InvalidLinkIndex {
        source_object_id: LegacyObjectId,
        endpoint: u8,
        target_object_id: LegacyObjectId,
        link_index: u16,
        target_link_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ReferenceResolutionSummary {
    pub object_lists: usize,
    pub object_ids: usize,
    pub connector_objects: usize,
    pub connected_endpoints: usize,
    pub resolved_endpoints: usize,
    pub unconnected_endpoints: usize,
    pub invalid_object_indices: usize,
    pub invalid_link_indices: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<ReferenceIssue>,
}

impl ReferenceResolutionSummary {
    pub fn is_clean(&self) -> bool {
        self.invalid_object_indices == 0 && self.invalid_link_indices == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ResolvedReferenceGraph {
    pub object_ids: Vec<LegacyObjectId>,
    pub endpoints: Vec<ResolvedEndpoint>,
    pub summary: ReferenceResolutionSummary,
}

fn target_link_count(object: &LegacyObject) -> usize {
    match object {
        LegacyObject::Text { .. } => 0,
        LegacyObject::Rectangle { custom_links, .. } => custom_links.as_ref().map_or(5, Vec::len),
        LegacyObject::Ellipse { .. } | LegacyObject::Flowchart { .. } => 5,
        LegacyObject::StraightLine { .. }
        | LegacyObject::ConnectorLine { .. }
        | LegacyObject::CurveLine { .. } => 2,
        LegacyObject::Bitmap { picture, .. }
        | LegacyObject::Metafile { picture, .. }
        | LegacyObject::InheritedLayer { picture, .. } => picture.links.len(),
        LegacyObject::Group { links, .. } => links.len(),
        LegacyObject::Polygon { points, .. } => points.len(),
    }
}

fn connector_links(object: &LegacyObject) -> Option<&[crate::object::LegacyLinkReference; 2]> {
    match object {
        LegacyObject::StraightLine { connector }
        | LegacyObject::ConnectorLine { connector, .. } => Some(&connector.links),
        LegacyObject::CurveLine {
            base: LegacyCurveBase::Connector { connector },
            ..
        } => Some(&connector.links),
        _ => None,
    }
}

fn object_id(list_path: &str, index: usize) -> LegacyObjectId {
    LegacyObjectId(format!("{list_path}/object/{index}"))
}

fn resolve_object_list(
    objects: &[LegacyObject],
    list_path: &str,
    graph: &mut ResolvedReferenceGraph,
) {
    graph.summary.object_lists += 1;

    let ids: Vec<_> = (0..objects.len())
        .map(|index| object_id(list_path, index))
        .collect();
    graph.summary.object_ids += ids.len();
    graph.object_ids.extend(ids.iter().cloned());

    for (source_index, object) in objects.iter().enumerate() {
        let source_id = ids[source_index].clone();

        if let Some(links) = connector_links(object) {
            graph.summary.connector_objects += 1;
            for (endpoint_offset, link) in links.iter().enumerate() {
                let endpoint = endpoint_offset as u8 + 1;
                let mut resolved_target = None;

                if link.object_index == -1 {
                    graph.summary.unconnected_endpoints += 1;
                } else {
                    graph.summary.connected_endpoints += 1;
                    if link.object_index < 0 || link.object_index as usize >= objects.len() {
                        graph.summary.invalid_object_indices += 1;
                        graph
                            .summary
                            .issues
                            .push(ReferenceIssue::InvalidObjectIndex {
                                source_object_id: source_id.clone(),
                                endpoint,
                                object_index: link.object_index,
                                owner_object_count: objects.len(),
                            });
                    } else {
                        let target_index = link.object_index as usize;
                        let target = &objects[target_index];
                        let target_id = ids[target_index].clone();
                        let link_index = link
                            .link_index
                            .expect("non--1 legacy object indices carry a serialized link index");
                        let link_count = target_link_count(target);

                        if link_index as usize >= link_count {
                            graph.summary.invalid_link_indices += 1;
                            graph.summary.issues.push(ReferenceIssue::InvalidLinkIndex {
                                source_object_id: source_id.clone(),
                                endpoint,
                                target_object_id: target_id,
                                link_index,
                                target_link_count: link_count,
                            });
                        } else {
                            graph.summary.resolved_endpoints += 1;
                            resolved_target = Some(ResolvedTarget {
                                object_id: target_id,
                                link_index,
                            });
                        }
                    }
                }

                graph.endpoints.push(ResolvedEndpoint {
                    source_object_id: source_id.clone(),
                    endpoint,
                    raw_object_index: link.object_index,
                    raw_link_index: link.link_index,
                    target: resolved_target,
                });
            }
        }

        if let LegacyObject::Group { children, .. } = object {
            let child_path = format!("{}/group", source_id.0);
            resolve_object_list(children, &child_path, graph);
        }
    }
}

/// Materialize stable importer identities and validate every legacy endpoint.
/// IDs are deterministic document-local paths. They are an importer boundary,
/// not the future DDNX element UUIDs.
pub fn resolve_container_reference_graph(container: &LegacyContainer) -> ResolvedReferenceGraph {
    let mut graph = ResolvedReferenceGraph::default();

    for (page_index, page) in container.pages.iter().enumerate() {
        for (layer_index, layer) in page.layers.iter().enumerate() {
            resolve_object_list(
                &layer.objects,
                &format!("page/{page_index}/layer/{layer_index}"),
                &mut graph,
            );
        }
    }

    if let Some(stencil) = &container.stencil {
        resolve_object_list(&stencil.objects, "stencil", &mut graph);
    }

    graph
}

/// Compact compatibility view used by `dd-migrate inspect`.
pub fn resolve_container_references(container: &LegacyContainer) -> ReferenceResolutionSummary {
    resolve_container_reference_graph(container).summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{object::parse_object_list, reader::LegacyReader};

    const DEFAULT_MARGIN: i32 = 1_778;
    const CL_WHITE: i32 = 0x00ff_ffff;
    const CL_NONE: i32 = 0x1fff_ffff;

    fn push_string16(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u16).to_le_bytes());
        target.extend_from_slice(value);
    }

    fn push_string32(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u32).to_le_bytes());
        target.extend_from_slice(value);
    }

    fn push_base(target: &mut Vec<u8>, name: &[u8]) {
        push_string16(target, name);
        for value in [10_i32, 20, 30, 40] {
            target.extend_from_slice(&value.to_le_bytes());
        }
        target.push(0);
    }

    fn push_text(target: &mut Vec<u8>, name: &[u8]) {
        push_base(target, name);
        push_string32(target, b"");
        target.push(0);
        target.push(0);
        target.extend_from_slice(&0_i32.to_le_bytes());
        target.extend_from_slice(&DEFAULT_MARGIN.to_le_bytes());
        target.extend_from_slice(&0_f32.to_le_bytes());
    }

    fn push_line(target: &mut Vec<u8>, name: &[u8]) {
        push_text(target, name);
        target.extend_from_slice(&666_i32.to_le_bytes());
        target.extend_from_slice(&0_i32.to_le_bytes());
    }

    fn push_shape(target: &mut Vec<u8>, name: &[u8]) {
        push_line(target, name);
        target.extend_from_slice(&CL_WHITE.to_le_bytes());
        target.extend_from_slice(&CL_NONE.to_le_bytes());
    }

    fn push_connector(target: &mut Vec<u8>, name: &[u8], object_index: i32, link_index: u16) {
        push_line(target, name);
        target.extend_from_slice(&0_u16.to_le_bytes());
        target.extend_from_slice(&0_u16.to_le_bytes());
        target.extend_from_slice(&0_u16.to_le_bytes());
        target.extend_from_slice(&CL_WHITE.to_le_bytes());
        target.extend_from_slice(&object_index.to_le_bytes());
        if object_index != -1 {
            target.extend_from_slice(&link_index.to_le_bytes());
        }
        target.extend_from_slice(&(-1_i32).to_le_bytes());
    }

    fn object_list_with_link(object_index: i32, link_index: u16) -> Vec<LegacyObject> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.push(2);
        push_shape(&mut bytes, b"Target");
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&(-1_i16).to_le_bytes());
        bytes.push(4);
        push_connector(&mut bytes, b"Line", object_index, link_index);

        let mut reader = LegacyReader::new(&bytes);
        let objects = parse_object_list(&mut reader, 28).unwrap();
        assert!(reader.is_eof());
        objects
    }

    fn container_with_objects(objects: Vec<LegacyObject>) -> LegacyContainer {
        use crate::container::{LegacyContainerDefaults, LegacyLayer, LegacyPage};
        LegacyContainer {
            defaults: LegacyContainerDefaults {
                default_font_name_raw: b"Arial".to_vec(),
                default_font_size: 12,
                default_font_style: 0,
                default_font_charset: 1,
                object_shadows: false,
                auto_line_break: true,
                connector_label_style: 1,
            },
            pages: vec![LegacyPage {
                width: 100,
                height: 100,
                name_raw: Vec::new(),
                layers: vec![LegacyLayer {
                    draw_color: -1,
                    objects,
                }],
            }],
            stencil: None,
            trailing_bytes: 0,
        }
    }

    #[test]
    fn materializes_valid_reference_and_raw_pair() {
        let graph =
            resolve_container_reference_graph(&container_with_objects(object_list_with_link(0, 4)));
        assert!(graph.summary.is_clean());
        assert_eq!(graph.object_ids.len(), 2);
        assert_eq!(graph.endpoints.len(), 2);
        assert_eq!(graph.summary.resolved_endpoints, 1);
        assert_eq!(graph.summary.unconnected_endpoints, 1);
        let endpoint = &graph.endpoints[0];
        assert_eq!(endpoint.raw_object_index, 0);
        assert_eq!(endpoint.raw_link_index, Some(4));
        assert_eq!(
            endpoint.target.as_ref().unwrap().object_id.0,
            "page/0/layer/0/object/0"
        );
    }

    #[test]
    fn diagnoses_out_of_range_link_index() {
        let graph =
            resolve_container_reference_graph(&container_with_objects(object_list_with_link(0, 5)));
        assert!(!graph.summary.is_clean());
        assert_eq!(graph.summary.invalid_link_indices, 1);
        assert!(graph.endpoints[0].target.is_none());
    }

    #[test]
    fn diagnoses_out_of_range_object_index() {
        let graph = resolve_container_reference_graph(&container_with_objects(
            object_list_with_link(99, 0),
        ));
        assert!(!graph.summary.is_clean());
        assert_eq!(graph.summary.invalid_object_indices, 1);
        assert!(matches!(
            graph.summary.issues.as_slice(),
            [ReferenceIssue::InvalidObjectIndex {
                object_index: 99,
                ..
            }]
        ));
    }
}
