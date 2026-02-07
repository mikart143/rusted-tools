use super::types::{ToolCallRequest, ToolCallResponse, ToolContent, ToolDefinition};
use crate::error::{ProxyError, Result};
use rmcp::model::{CallToolRequestParams, PaginatedRequestParams, RawContent};
use rmcp::service::{RoleClient, RunningService};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, error};

const REQUEST_BUFFER: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeState {
    Running,
    Stopped,
    Failed(String),
}

#[derive(Clone)]
pub(crate) struct McpRuntimeHandle {
    tx: mpsc::Sender<ServiceRequest>,
    state: Arc<RwLock<RuntimeState>>,
    join: Arc<Mutex<Option<JoinHandle<()>>>>,
}

enum ServiceRequest {
    ListTools {
        resp: oneshot::Sender<Result<Vec<ToolDefinition>>>,
    },
    CallTool {
        request: ToolCallRequest,
        resp: oneshot::Sender<Result<ToolCallResponse>>,
    },
    Stop {
        resp: oneshot::Sender<Result<()>>,
    },
}

pub(crate) fn spawn_runtime(
    server_name: String,
    service: RunningService<RoleClient, ()>,
) -> McpRuntimeHandle {
    let (tx, mut rx) = mpsc::channel(REQUEST_BUFFER);
    let state = Arc::new(RwLock::new(RuntimeState::Running));
    let state_clone = Arc::clone(&state);

    let join = tokio::spawn(async move {
        let mut service = service;

        loop {
            match rx.recv().await {
                Some(ServiceRequest::ListTools { resp }) => {
                    let result = list_tools_from_service(&server_name, &service).await;
                    let _ = resp.send(result);
                }
                Some(ServiceRequest::CallTool { request, resp }) => {
                    let result = call_tool_on_service(&server_name, &service, request).await;
                    let _ = resp.send(result);
                }
                Some(ServiceRequest::Stop { resp }) => {
                    let result = service
                        .close()
                        .await
                        .map(|_| ())
                        .map_err(ProxyError::mcp_client_stop_failed);
                    set_state(&state_clone, &result).await;
                    let _ = resp.send(result);
                    break;
                }
                None => {
                    let result = service
                        .close()
                        .await
                        .map(|_| ())
                        .map_err(ProxyError::mcp_client_stop_failed);
                    set_state(&state_clone, &result).await;
                    break;
                }
            }
        }
    });

    McpRuntimeHandle {
        tx,
        state,
        join: Arc::new(Mutex::new(Some(join))),
    }
}

impl McpRuntimeHandle {
    pub(crate) async fn state(&self) -> RuntimeState {
        self.state.read().await.clone()
    }

    pub(crate) async fn list_tools(&self, server_name: &str) -> Result<Vec<ToolDefinition>> {
        self.ensure_running(server_name).await?;

        let (resp_tx, resp_rx) = oneshot::channel();
        if self
            .tx
            .send(ServiceRequest::ListTools { resp: resp_tx })
            .await
            .is_err()
        {
            return Err(self
                .runtime_failed(server_name, "worker channel closed")
                .await);
        }

        resp_rx
            .await
            .map_err(|_| ProxyError::mcp_cancelled("list tools", server_name))?
    }

    pub(crate) async fn call_tool(
        &self,
        server_name: &str,
        request: ToolCallRequest,
    ) -> Result<ToolCallResponse> {
        self.ensure_running(server_name).await?;

        let (resp_tx, resp_rx) = oneshot::channel();
        if self
            .tx
            .send(ServiceRequest::CallTool {
                request,
                resp: resp_tx,
            })
            .await
            .is_err()
        {
            return Err(self
                .runtime_failed(server_name, "worker channel closed")
                .await);
        }

        resp_rx
            .await
            .map_err(|_| ProxyError::mcp_cancelled("call tool", server_name))?
    }

    pub(crate) async fn stop(&self, server_name: &str) -> Result<()> {
        self.ensure_running(server_name).await?;

        let (resp_tx, resp_rx) = oneshot::channel();
        if self
            .tx
            .send(ServiceRequest::Stop { resp: resp_tx })
            .await
            .is_err()
        {
            return Err(self
                .runtime_failed(server_name, "worker channel closed")
                .await);
        }

        resp_rx
            .await
            .map_err(|_| ProxyError::mcp_cancelled("stop", server_name))??;

        let mut join_lock = self.join.lock().await;
        if let Some(join_handle) = join_lock.take()
            && let Err(err) = join_handle.await
        {
            let _ = self
                .runtime_failed(server_name, &format!("worker panicked: {}", err))
                .await;
            return Err(ProxyError::server_runtime_failed(
                server_name,
                format!("worker panicked: {}", err),
            ));
        }

        Ok(())
    }

    async fn ensure_running(&self, server_name: &str) -> Result<()> {
        match self.state.read().await.clone() {
            RuntimeState::Running => Ok(()),
            RuntimeState::Stopped => Err(ProxyError::server_not_running(server_name)),
            RuntimeState::Failed(details) => {
                Err(ProxyError::server_runtime_failed(server_name, details))
            }
        }
    }

    async fn runtime_failed(&self, server_name: &str, details: &str) -> ProxyError {
        let message = details.to_string();
        let mut state = self.state.write().await;
        *state = RuntimeState::Failed(message.clone());
        ProxyError::server_runtime_failed(server_name, message)
    }
}

async fn set_state(state: &Arc<RwLock<RuntimeState>>, result: &Result<()>) {
    let mut state_lock = state.write().await;
    match result {
        Ok(()) => *state_lock = RuntimeState::Stopped,
        Err(err) => *state_lock = RuntimeState::Failed(err.to_string()),
    }
}

async fn list_tools_from_service(
    server_name: &str,
    service: &RunningService<RoleClient, ()>,
) -> Result<Vec<ToolDefinition>> {
    debug!("Listing tools for server: {}", server_name);

    let mut tool_list = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let request = Some(PaginatedRequestParams {
            meta: None,
            cursor: cursor.clone(),
        });

        match service.list_tools(request).await {
            Ok(result) => {
                tool_list.extend(result.tools.into_iter().map(|t| ToolDefinition {
                    name: t.name.to_string(),
                    description: t.description.map(|d| d.to_string()),
                    input_schema: Value::Object((*t.input_schema).clone()),
                }));

                cursor = result.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            Err(e) => {
                error!("Failed to list tools for {}: {}", server_name, e);
                return Err(ProxyError::mcp_service_error("list tools", e));
            }
        }
    }

    debug!(
        "Found {} tools for server: {}",
        tool_list.len(),
        server_name
    );
    Ok(tool_list)
}

async fn call_tool_on_service(
    server_name: &str,
    service: &RunningService<RoleClient, ()>,
    request: ToolCallRequest,
) -> Result<ToolCallResponse> {
    debug!("Calling tool '{}' on server: {}", request.name, server_name);

    let mcp_request = CallToolRequestParams {
        meta: None,
        name: request.name.clone().into(),
        arguments: request.arguments.as_object().cloned(),
        task: None,
    };

    match service.call_tool(mcp_request).await {
        Ok(result) => {
            let response_content: Vec<ToolContent> = result
                .content
                .into_iter()
                .filter_map(|c| match c.raw {
                    RawContent::Text(text_content) => Some(ToolContent::Text {
                        text: text_content.text,
                    }),
                    RawContent::Image(image_content) => Some(ToolContent::Image {
                        data: image_content.data,
                        mime_type: image_content.mime_type,
                    }),
                    RawContent::Resource(resource_content) => match resource_content.resource {
                        rmcp::model::ResourceContents::TextResourceContents {
                            uri,
                            mime_type,
                            ..
                        } => Some(ToolContent::Resource { uri, mime_type }),
                        rmcp::model::ResourceContents::BlobResourceContents {
                            uri,
                            mime_type,
                            ..
                        } => Some(ToolContent::Resource { uri, mime_type }),
                    },
                    _ => None,
                })
                .collect();

            Ok(ToolCallResponse {
                content: response_content,
                is_error: result.is_error,
            })
        }
        Err(e) => {
            error!(
                "Failed to call tool '{}' on {}: {}",
                request.name, server_name, e
            );
            Err(ProxyError::mcp_service_error("call tool", e))
        }
    }
}
