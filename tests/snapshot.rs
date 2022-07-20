use cleora::configuration::{Column, Configuration, FileType, OutputFormat};
use cleora::embedding::{calculate_embeddings, calculate_embeddings_mmap};
use cleora::persistence::embedding::EmbeddingPersistor;
use cleora::persistence::entity::InMemoryEntityMappingPersistor;
use cleora::pipeline::build_graphs;
use insta::assert_debug_snapshot;
use std::sync::Arc;
use std::{fmt, io};

/// This test performs work for sample case and saves snapshot file.
/// Snapshot testing takes advantage of deterministic character of Cleora.
/// Any discrepancies between original snapshot results and current ones can be then
/// reviewed along with the code which introduced discrepancy.
///
/// Differing snapshot has to be renamed by removing .new from the name.
/// For more information, please review https://crates.io/crates/insta
///
/// Code executed performs roughly the same work as:
/// ./cleora -i files/samples/edgelist_1.tsv --columns="complex::reflexive::a b complex::c"
/// -d 128 -n 4 --relation-name=R1 -p 0
#[test]
fn test_build_graphs_and_create_embeddings() {
    let config = prepare_config();

    let in_memory_entity_mapping_persistor = InMemoryEntityMappingPersistor::default();
    let in_memory_entity_mapping_persistor = Arc::new(in_memory_entity_mapping_persistor);

    // build sparse matrices
    let sparse_matrices = build_graphs(&config, in_memory_entity_mapping_persistor.clone());
    let in_memory_entity_mapping_persistor =
        Arc::try_unwrap(in_memory_entity_mapping_persistor).expect("All other references dropped");

    let config = Arc::new(config);

    // embeddings for in-memory and mmap files calculation should be the same
    for sparse_matrix in sparse_matrices.into_iter() {
        let sparse_matrix = Arc::new(sparse_matrix);
        let snapshot_name = format!(
            "embeddings_{}_{}",
            sparse_matrix.descriptor.col_a_name, sparse_matrix.descriptor.col_b_name
        );

        let mut in_memory_embedding_persistor = InMemoryEmbeddingPersistor::default();
        // calculate embeddings in memory
        calculate_embeddings(
            config.clone(),
            sparse_matrix.clone(),
            &in_memory_entity_mapping_persistor,
            &mut in_memory_embedding_persistor,
        );
        in_memory_embedding_persistor
            .entities
            .sort_by_key(|e| e.entity.clone());

        assert_debug_snapshot!(snapshot_name.clone(), in_memory_embedding_persistor);

        let mut in_memory_embedding_persistor = InMemoryEmbeddingPersistor::default();
        // calculate embeddings with mmap files
        calculate_embeddings_mmap(
            config.clone(),
            sparse_matrix.clone(),
            &in_memory_entity_mapping_persistor,
            &mut in_memory_embedding_persistor,
        );
        in_memory_embedding_persistor
            .entities
            .sort_by_key(|e| e.entity.clone());

        assert_debug_snapshot!(snapshot_name, in_memory_embedding_persistor);
    }
}

fn prepare_config() -> Configuration {
    let columns = vec![
        Column {
            name: "a".to_string(),
            complex: true,
            reflexive: true,
            ..Column::default()
        },
        Column {
            name: "b".to_string(),
            ..Column::default()
        },
        Column {
            name: "c".to_string(),
            complex: true,
            ..Column::default()
        },
    ];

    let config = Configuration {
        produce_entity_occurrence_count: true,
        embeddings_dimension: 128,
        max_number_of_iteration: 4,
        seed: None,
        prepend_field: false,
        log_every_n: 10000,
        in_memory_embedding_calculation: true,
        input: vec!["files/samples/edgelist_1.tsv".to_string()],
        file_type: FileType::Tsv,
        output_format: OutputFormat::TextFile,
        output_dir: None,
        relation_name: "r1".to_string(),
        columns,
    };
    config
}

#[derive(Debug, Default)]
struct InMemoryEmbeddingPersistor {
    entity_count: u32,
    dimenstion: u16,
    entities: Vec<InMemoryEntity>,
}

struct InMemoryEntity {
    entity: String,
    occur_count: u32,
    vector: Vec<f32>,
}

/// Custom implementation used so we can round up floats in vector
/// Depending on order of floating operations we might have different results on the last digits
/// Print floats up to 3 digits to not hard-wire our snapshot tests to specific operation order
impl fmt::Debug for InMemoryEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "InMemoryEntity {{")?;
        writeln!(f, "    entity: \"{}\",", self.entity)?;
        writeln!(f, "    occur_count: {},", self.occur_count)?;
        writeln!(f, "    vector: [")?;
        for val in &self.vector {
            writeln!(f, "        {:.3?},", val)?;
        }
        writeln!(f, "    ],")?;
        write!(f, "}}")?;
        Ok(())
    }
}

impl EmbeddingPersistor for InMemoryEmbeddingPersistor {
    fn put_metadata(&mut self, entity_count: u32, dimension: u16) -> Result<(), io::Error> {
        self.entity_count = entity_count;
        self.dimenstion = dimension;
        Ok(())
    }
    fn put_data(
        &mut self,
        entity: &str,
        occur_count: u32,
        vector: Vec<f32>,
    ) -> Result<(), io::Error> {
        let entity = entity.to_string();
        self.entities.push(InMemoryEntity {
            entity,
            occur_count,
            vector,
        });
        Ok(())
    }
    fn finish(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}
