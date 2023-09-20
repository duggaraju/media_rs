#[cfg(target_os = "unix")]
use std::os::unix::prelude::{IntoRawFd, RawFd};
use tokio_pipe::PipeRead;

use bytes::BytesMut;
use futures::Stream;
use k8s_openapi::{api::batch::v1::Job, serde_json};
use kube::{
    api::PostParams,
    runtime::wait::{await_condition, conditions},
    Api, Client,
};
use log::{info, trace};
use storage::StorageContainer;
// use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::azure_storage::StorageServer;
use crate::manifest::{SEGMENT_DURATION, VARIANTS};
use crate::{
    azure_storage::AzureStorage,
    config::{AppConfig, JobConfig},
};

#[derive(Clone)]
pub struct KubernetesMediaServer {
    client: Client,
    config: AppConfig,
    job_config: JobConfig,
    storage: AzureStorage,
}

impl KubernetesMediaServer {
    pub async fn new(config: AppConfig, job_config: JobConfig, storage: AzureStorage) -> Self {
        let client = Client::try_default().await.unwrap();
        KubernetesMediaServer {
            client,
            config,
            job_config,
            storage,
        }
    }

    pub async fn get_media_segment(
        &self,
        container: &str,
        video: &str,
        level: u32,
        segment: u32,
    ) -> anyhow::Result<Box<dyn Stream<Item = azure_core::Result<bytes::Bytes>> + Send>> {
        //remove the extension from the file name to create the blob path.
        let path = std::path::Path::new(&video)
            .file_stem()
            .map(|s| s.to_str().unwrap())
            .unwrap();

        let blob_name = format!("/{}/level{}/segment{}.ts", path, level, segment);
        let result = self.storage.get_media_file(container, &blob_name).await;
        if let Ok(stream) = result {
            return Ok(Box::new(stream));
        }

        let mut pipes: Option<(PipeRead, RawFd)> = None;
        if self.config.stream_while_encoding && !self.config.encode_ahead {
            let p = tokio_pipe::pipe()?;
            pipes = Some((p.0, p.1.into_raw_fd()));
        }

        let pipe_name = pipes.as_ref().map(|(_, w)| w.to_string());
        self.submit_job(container, video, level, segment, pipe_name)
            .await?;
        if self.config.stream_while_encoding {
            let (reader, writer) = pipes.unwrap();
            let buf = BytesMut::with_capacity(64000);
            let stream =
                futures::stream::unfold((reader, writer, buf), |(mut r, w, mut b)| async move {
                    let res = r.read(&mut b).await;
                    match res {
                        Ok(size) => {
                            info!("read {} bytes from pipe", size);
                            if size == 0 {
                                return None;
                            }
                            Some((Ok(b.split().freeze()), (r, w, b)))
                        }
                        Err(err) => Some((
                            Err(azure_core::Error::new(azure_storage::ErrorKind::Io, err)),
                            (r, w, b),
                        )),
                    }
                });
            Ok(Box::new(stream))
        } else {
            let stream = self.storage.get_media_file(container, &blob_name).await?;
            Ok(Box::new(stream))
        }
    }

    fn get_job_name(&self, video: &str, level: u32, segment: u32) -> String {
        if self.config.encode_ahead {
            format!("{}-l{}", video.replace('_', "-"), level).to_lowercase()
        } else if self.config.cache_fragments {
            format!("{}-l{}-s{}", video.replace('_', "-"), level, segment).to_lowercase()
        } else {
            Uuid::new_v4()
                .hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string()
        }
    }

    async fn submit_job(
        &self,
        container: &str,
        video: &str,
        level: u32,
        segment: u32,
        pipe_name: Option<String>,
    ) -> anyhow::Result<()> {
        let url = self
            .storage
            .get_sas_url(container, video, pipe_name.is_some())?;
        let job_name = self.get_job_name(video, level, segment);
        let jobs: Api<Job> = Api::default_namespaced(self.client.clone());
        let image_name: &str = if self.config.use_gpu {
            self.job_config.gpu_image_name.as_str()
        } else {
            self.job_config.image_name.as_str()
        };
        let image = format!("{}{}", self.job_config.registry_name, image_name);
        let mut args: Vec<String> = Vec::new();
        args.push(String::from("-i"));
        args.push(url.to_string());
        args.push("-o".to_string());
        if pipe_name.is_some() {
            args.push(format!(
                "http://{}/pipe/{}",
                self.config.pod_address,
                pipe_name.as_ref().unwrap()
            ));
        } else {
            args.push(format!("{url}/level{}/segment%d.ts", level));
        }
        args.push("-t".to_string());
        args.push((segment * crate::manifest::SEGMENT_DURATION).to_string());
        if !self.config.encode_ahead {
            args.push("-d".to_string());
            args.push(SEGMENT_DURATION.to_string());
        }
        args.push("-s".to_string());
        args.push(format!(
            "{}x{}",
            VARIANTS[level as usize].width, VARIANTS[level as usize].height
        ));
        args.push("-b".to_string());
        args.push(VARIANTS[level as usize].bandwidth.to_string());
        if self.config.use_gpu {
            args.push("-g".to_string());
        }

        let data = serde_json::from_value(serde_json::json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": {
                "name": job_name,
            },
            "spec": {
                "ttlSecondsAfterFinished": 30,
                "template": {
                    "spec": {
                        "restartPolicy": "Never",
                        "tolerations": [{
                            "key": "type",
                            "value": "gpubackend",
                            "effect": "NoSchedule"
                        }],
                        "containers": [{
                            "name": "ffmpeg",
                            "image": image,
                            "imagePullPolicy": "IfNotPresent",
                            "resources": {
                                "limits": {
                                }
                            },
                            "args": args
                        }]
                    }
                }
            }
        }))?;
        let result = jobs.create(&PostParams::default(), &data).await;
        trace!("Job result is {:?}", result);

        if let Ok(job) = result {
            info!(
                "Successfully created job: {}",
                job.metadata.name.as_ref().unwrap()
            );

            if pipe_name.is_none() {
                info!("Waiting for job to complete {}", job.metadata.name.unwrap());
                let cond = await_condition(jobs.clone(), &job_name, conditions::is_job_completed());
                let _ = tokio::time::timeout(std::time::Duration::from_secs(20), cond).await;
            }
        } else {
            info!(
                "Failed to create job. Probably another job runing {:?}",
                result
            );
        }
        Ok(())
    }

    async fn wait_for_job(&self) {
        let jobs: Api<Job> = Api::default_namespaced(self.client.clone());
        //await jobs.watch(lp, version)
    }
}
