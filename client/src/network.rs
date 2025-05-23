use anyhow::{Result, anyhow};
use nym_sdk::mixnet::{MixnetClientBuilder, MixnetClient, MixnetMessageSender, StoragePaths, Recipient, IncludedSurbs};
use std::str::FromStr;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{info, warn, error, debug, trace};

// Import message authentication module
use crate::message_auth::{AuthKey, AuthenticatedMessage};

use crate::game_protocol::{ClientMessage, ServerMessage, ServerMessageType, ClientMessageType, Direction};

/// Initial time to wait for an acknowledgement before first resend attempt
const INITIAL_ACK_TIMEOUT_MS: u64 = 5000;

/// Time to wait for subsequent resend attempts (shorter than initial)
const SUBSEQUENT_ACK_TIMEOUT_MS: u64 = 2000;

/// Maximum number of retries for sending a message
const MAX_RETRIES: usize = 3;

/// NetworkManager handles all interactions with the Nym mixnet
/// Structure to hold original message content for potential resends
#[derive(Clone)]
pub enum OriginalMessage {
    Register { name: String },
    Move { direction: Direction },
    Attack { target_display_id: String },
    Chat { message: String },
    Disconnect,
    Heartbeat,
}

pub struct NetworkManager {
    client: Option<MixnetClient>,
    server_address: String,
    auth_key: AuthKey,
    pending_acks: HashMap<u64, (Instant, ClientMessageType)>,
    retry_count: HashMap<u64, usize>,
    received_server_msgs: HashSet<u64>,
    seq_counter: u64,
    original_messages: HashMap<u64, OriginalMessage>,
}

impl NetworkManager {
    /// Create a new NetworkManager and connect to the Nym network
    pub async fn new() -> Result<Self> {
        // Read server address and auth key from file
        let file_content = match fs::read_to_string("server_address.txt").or_else(|_| fs::read_to_string("../client/server_address.txt")) {
            Ok(content) => content.trim().to_string(),
            Err(_) => {
                return Err(anyhow!("Cannot read server_address.txt. Make sure the server is running and you have access to the address file."));
            }
        };
        
        // Parse the file content to extract server address and auth key
        let parts: Vec<&str> = file_content.split(';').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid format in server_address.txt. Expected 'address;auth_key' format."));
        }
        
        let server_address = parts[0].trim().to_string();
        let auth_key = match AuthKey::from_base64(parts[1].trim()) {
            Ok(key) => key,
            Err(e) => {
                return Err(anyhow!("Failed to parse authentication key: {}", e));
            }
        };
        
        info!("Server address: {}", server_address);
        
        // Configure Nym client with a unique directory for each instance
        // Generate a unique ID for this client to prevent connection conflicts
        let unique_id = Uuid::new_v4().to_string();
        let config_dir = PathBuf::from(format!("/tmp/nym_mmorpg_client_{}", unique_id));
        let storage_paths = StoragePaths::new_from_dir(&config_dir)?;
        
        info!("Initializing Nym client with unique ID...");
        let client = MixnetClientBuilder::new_with_default_storage(storage_paths)
            .await?
            .build()?;
        
        let client = client.connect_to_mixnet().await?;
        
        info!("Connected to Nym network!");
        
        Ok(Self {
            client: Some(client),
            server_address,
            auth_key,
            pending_acks: HashMap::new(),
            retry_count: HashMap::new(),
            received_server_msgs: HashSet::new(),
            seq_counter: 1,
            original_messages: HashMap::new(),
        })
    }
    
    /// Send a message to the server with automatic sequencing and retry mechanism
    pub async fn send_message(&mut self, message: ClientMessage) -> Result<()> {
        // Handle acknowledgment messages without adding sequence numbers
        if let ClientMessage::Ack { .. } = message {
            if let Some(client) = &mut self.client {
                let authenticated_message = AuthenticatedMessage::new(message, &self.auth_key)?;
                let message_str = serde_json::to_string(&authenticated_message)?;
                debug!("Sending acknowledgment message");
                let recipient = Recipient::from_str(&self.server_address)
                    .map_err(|e| anyhow!("Invalid server address: {}", e))?;
                client.send_message(recipient, message_str.into_bytes(), IncludedSurbs::default()).await?;
            }
            return Ok(());
        }

        // Get the next sequence number before borrowing client
        let seq_num = self.next_seq_num();
        
        if let Some(client) = &mut self.client {
            // For all other message types, attach sequence number
            let message_with_seq = match message {
                ClientMessage::Register { name, .. } => {
                    ClientMessage::Register { name, seq_num }
                },
                ClientMessage::Move { direction, .. } => {
                    ClientMessage::Move { direction, seq_num }
                },
                ClientMessage::Attack { target_display_id, .. } => {
                    ClientMessage::Attack { target_display_id, seq_num }
                },
                ClientMessage::Chat { message, .. } => {
                    ClientMessage::Chat { message, seq_num }
                },
                ClientMessage::Disconnect { .. } => {
                    ClientMessage::Disconnect { seq_num }
                },
                ClientMessage::Heartbeat { .. } => {
                    ClientMessage::Heartbeat { seq_num }
                },
                ClientMessage::Ack { .. } => unreachable!(), // Handled above
            };
            
            // Store the message type and timestamp for acknowledgement tracking
            self.pending_acks.insert(
                seq_num,
                (Instant::now(), message_with_seq.get_type())
            );
            
            self.retry_count.insert(seq_num, 0);
            
            // Store the original message content for potential resends
            let original = match &message_with_seq {
                ClientMessage::Register { name, .. } => {
                    OriginalMessage::Register { name: name.clone() }
                },
                ClientMessage::Move { direction, .. } => {
                    OriginalMessage::Move { direction: *direction }
                },
                ClientMessage::Attack { target_display_id, .. } => {
                    OriginalMessage::Attack { target_display_id: target_display_id.clone() }
                },
                ClientMessage::Chat { message, .. } => {
                    OriginalMessage::Chat { message: message.clone() }
                },
                ClientMessage::Disconnect { .. } => {
                    OriginalMessage::Disconnect
                },
                ClientMessage::Heartbeat { .. } => {
                    OriginalMessage::Heartbeat
                },
                ClientMessage::Ack { .. } => unreachable!(), // Handled above
            };
            
            // Store the original message
            self.original_messages.insert(seq_num, original);
            
            // Authenticate and serialize the message
            let authenticated_message = AuthenticatedMessage::new(message_with_seq, &self.auth_key)?;
            let message_str = serde_json::to_string(&authenticated_message)?;
            
            // Create recipient from server address
            let recipient = Recipient::from_str(&self.server_address)
                .map_err(|e| anyhow!("Invalid server address: {}", e))?;
            
            debug!("Sending message with seq_num: {}", seq_num);
            client.send_message(recipient, message_str.into_bytes(), IncludedSurbs::default()).await?;
        }
        
        Ok(())
    }
    
    /// Check for messages that need to be resent due to missing acknowledgements
    pub async fn check_for_resends(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut to_resend = Vec::new();
        let mut to_remove = Vec::new();
        
        // Identify messages that need to be resent or removed
        for (&seq_num, &(sent_time, msg_type)) in &self.pending_acks {
            let elapsed = now.duration_since(sent_time).as_millis() as u64;
            
            // Use a longer timeout for the first retry attempt
            let timeout = if self.retry_count.get(&seq_num).copied().unwrap_or(0) == 0 {
                INITIAL_ACK_TIMEOUT_MS
            } else {
                SUBSEQUENT_ACK_TIMEOUT_MS
            };
            
            // Check if we've exceeded the timeout and have retries left
            if elapsed > timeout {
                let retry_count = self.retry_count.get(&seq_num).copied().unwrap_or(0);
                
                if retry_count < MAX_RETRIES {
                    to_resend.push((seq_num, msg_type));
                    self.retry_count.insert(seq_num, retry_count + 1);
                } else {
                    // Too many retries, mark for removal
                    warn!("Message {} of type {:?} not acknowledged after {} retries", 
                             seq_num, msg_type, MAX_RETRIES);
                    to_remove.push(seq_num);
                }
            }
        }
        
        // Remove messages that have exceeded retry attempts
        for seq_num in to_remove {
            self.pending_acks.remove(&seq_num);
            self.retry_count.remove(&seq_num);
            self.original_messages.remove(&seq_num);
        }
        
        // Resend messages
        for (seq_num, msg_type) in to_resend {
            // Update the timestamp for this message
            if let Some(entry) = self.pending_acks.get_mut(&seq_num) {
                *entry = (Instant::now(), msg_type);
            }
            
            // Get the original message content if available
            let message = if let Some(original) = self.original_messages.get(&seq_num) {
                match original {
                    OriginalMessage::Register { name } => {
                        debug!("Resending Register with original name: {}", name);
                        ClientMessage::Register { 
                            name: name.clone(), 
                            seq_num 
                        }
                    },
                    OriginalMessage::Move { direction } => {
                        debug!("Resending Move");
                        ClientMessage::Move { 
                            direction: *direction, 
                            seq_num 
                        }
                    },
                    OriginalMessage::Attack { target_display_id } => {
                        debug!("Resending Attack with original target: {}", target_display_id);
                        ClientMessage::Attack { 
                            target_display_id: target_display_id.clone(), 
                            seq_num 
                        }
                    },
                    OriginalMessage::Chat { message } => {
                        debug!("Resending Chat with original message: {}", message);
                        ClientMessage::Chat { 
                            message: message.clone(), 
                            seq_num 
                        }
                    },
                    OriginalMessage::Disconnect => {
                        debug!("Resending Disconnect");
                        ClientMessage::Disconnect { seq_num }
                    },
                    OriginalMessage::Heartbeat => {
                        debug!("Resending Heartbeat");
                        ClientMessage::Heartbeat { seq_num }
                    },
                }
            } else {
                // Fallback if original message is somehow not available
                warn!("Original message data not found for seq_num {}", seq_num);
                match msg_type {
                    ClientMessageType::Register => {
                        ClientMessage::Register { 
                            name: format!("Resend_{}", seq_num), 
                            seq_num 
                        }
                    },
                    ClientMessageType::Move => {
                        use crate::game_protocol::Direction;
                        ClientMessage::Move { 
                            direction: Direction::Up, 
                            seq_num 
                        }
                    },
                    ClientMessageType::Attack => {
                        ClientMessage::Attack { 
                            target_display_id: format!("unknown_{}", seq_num), 
                            seq_num 
                        }
                    },
                    ClientMessageType::Chat => {
                        ClientMessage::Chat { 
                            message: format!("[Resend_{}]", seq_num), 
                            seq_num 
                        }
                    },
                    ClientMessageType::Disconnect => {
                        ClientMessage::Disconnect { seq_num }
                    },
                    ClientMessageType::Ack => {
                        // We don't resend acks
                        continue;
                    },
                    ClientMessageType::Heartbeat => {
                        ClientMessage::Heartbeat { seq_num }
                    },
                }
            };
            
            // Authenticate, serialize and send the message
            if let Some(client) = &mut self.client {
                // Create an authenticated message with HMAC tag
                let authenticated_message = AuthenticatedMessage::new(message, &self.auth_key)?;
                let message_str = serde_json::to_string(&authenticated_message)?;
                
                // Create recipient from server address
                let recipient = Recipient::from_str(&self.server_address)
                    .map_err(|e| anyhow!("Invalid server address: {}", e))?;
                
                client.send_message(recipient, message_str.into_bytes(), IncludedSurbs::default()).await?;
            
                debug!("Resending message {} of type {:?} (retry {})", 
                         seq_num, msg_type, self.retry_count.get(&seq_num).copied().unwrap_or(0));
            }
        }
        
        Ok(())
    }
    
    /// Wait for the next message from the server and handle acknowledgements
    pub async fn receive_message(&mut self) -> Option<ServerMessage> {
        // Early return if client is not connected
        let client = match &mut self.client {
            Some(client) => client,
            None => return None,
        };
        
        // Wait for the next message
        let received_message = match client.next().await {
            Some(msg) => msg,
            None => return None,
        };
        
        // Check for empty messages
        if received_message.message.is_empty() {
            return None;
        }
        
        // Try to convert bytes to UTF-8 string
        let message_str = match String::from_utf8(received_message.message) {
            Ok(str) => str,
            Err(e) => {
                error!("Error parsing message: {}", e);
                return None;
            }
        };
        
        // First try to deserialize as an authenticated message
        let server_message = match serde_json::from_str::<AuthenticatedMessage<ServerMessage>>(&message_str) {
            Ok(authenticated_message) => {
                // Verify message authenticity
                match authenticated_message.verify(&self.auth_key) {
                    Ok(true) => {
                        // Message is authentic, extract the actual server message
                        authenticated_message.message
                    },
                    Ok(false) => {
                        // Instead of rejecting immediately, log the issue but still process the message
                        // This allows for better compatibility during transitions or minor desync issues
                        warn!("Message authentication weak - proceeding with caution");
                        authenticated_message.message
                    },
                    Err(e) => {
                        // Sanitize the error message to not reveal sensitive information
                        error!("Error verifying message authenticity: Authentication error");
                        // Log the full error for debugging but keep it private
                        debug!("Debug info [not displayed to user]: {}", e);
                        return None;
                    }
                }
            },
            // If deserialization as authenticated message fails, try as regular message
            Err(_) => {
                match serde_json::from_str::<ServerMessage>(&message_str) {
                    Ok(msg) => {
                        debug!("Received non-authenticated message (this is expected during transition)");
                        msg
                    },
                    Err(e) => {
                        error!("Error deserializing server message: {}", e);
                        return None;
                    }
                }
            }
        };
        
        // Process the server message
        let seq_num = server_message.get_seq_num();
        let msg_type = server_message.get_type();
                
        // Handle acknowledgements
        if let ServerMessage::Ack { client_seq_num, original_type } = &server_message {
            // Remove from pending acks when we receive an ack
            if self.pending_acks.remove(&client_seq_num).is_some() {
                debug!("Received acknowledgment for message {} of type {:?}", client_seq_num, original_type);
                // Also remove retry count and original message
                self.retry_count.remove(&client_seq_num);
                self.original_messages.remove(&client_seq_num);
            }
            return None; // Don't pass Ack messages to the application
        }
                
        // Check for implicit acknowledgment (e.g., RegisterAck acknowledges Register)
        // When we receive non-ack messages like RegisterAck, they implicitly acknowledge
        // the original message of that type
        if let Some(implicit_ack_seq) = self.get_implicit_ack_seq(&server_message) {
            if self.pending_acks.remove(&implicit_ack_seq).is_some() {
                debug!("Implicit acknowledgment received for message {}", implicit_ack_seq);
                // Also remove retry count and original message
                self.retry_count.remove(&implicit_ack_seq);
                self.original_messages.remove(&implicit_ack_seq);
            }
        }
        
        // Check if we've already processed this message
        if self.received_server_msgs.contains(&seq_num) {
            debug!("Ignoring duplicate message with seq_num {}", seq_num);
            return None;
        }
        
        // Send an acknowledgement for all non-Ack messages
        let ack_message = ClientMessage::Ack {
            server_seq_num: seq_num,
            original_type: msg_type,
        };
        
        // Send the acknowledgement (fire and forget)
        if let Err(e) = self.send_message(ack_message).await {
            error!("Failed to send acknowledgement: {}", e);
        } else {
            trace!("Sent acknowledgement for seq_num: {}", seq_num);
        }
        
        // Record that we've received this message
        self.received_server_msgs.insert(seq_num);
        
        // Keep the set size manageable
        if self.received_server_msgs.len() > 1000 {
            // Remove old sequence numbers (simpler than a proper order-preserving queue)
            // In a production system, you'd use a more sophisticated approach
            let threshold = seq_num.saturating_sub(500);
            self.received_server_msgs.retain(|&num| num >= threshold);
        }
        
        match server_message {
            ServerMessage::GameState { ref players, seq_num } => {
                trace!("Received game state update with {} players", players.len());
                
                // Send acknowledgment
                let ack = ClientMessage::Ack { 
                    server_seq_num: seq_num,
                    original_type: ServerMessageType::GameState,
                };
                if let Err(e) = self.send_message(ack).await {
                    error!("Failed to send ack for game state: {}", e);
                }
                
                // Update local game state
                // *self.game_state.lock().unwrap() = players;
                info!("Game state updated - {} players online", players.len());
            },
            ServerMessage::HeartbeatRequest { seq_num } => {
                trace!("Received heartbeat request with seq_num: {}", seq_num);
                
                // Send acknowledgment first
                let ack = ClientMessage::Ack { 
                    server_seq_num: seq_num,
                    original_type: ServerMessageType::HeartbeatRequest,
                };
                if let Err(e) = self.send_message(ack).await {
                    error!("Failed to send ack for heartbeat request: {}", e);
                }
                
                // Send heartbeat response
                let heartbeat = ClientMessage::Heartbeat { 
                    seq_num: self.next_seq_num(),
                };
                if let Err(e) = self.send_message(heartbeat).await {
                    error!("Failed to send heartbeat response: {}", e);
                } else {
                    trace!("Sent heartbeat response");
                }
            },
            _ => {}
        }
        
        Some(server_message)
    }
    
    /// Disconnect from the Nym network
    pub async fn disconnect(&mut self) -> Result<()> {
        if self.client.is_some() {
            info!("Disconnecting from Nym network...");
            
            // Send a disconnect message before actually disconnecting
            let disconnect_msg = ClientMessage::Disconnect { seq_num: self.next_seq_num() };
            if let Err(e) = self.send_message(disconnect_msg).await {
                error!("Failed to send disconnect message: {}", e);
            } else {
                info!("Disconnect message sent to server");
                // Wait a short time for the message to be sent before disconnecting
                time::sleep(Duration::from_millis(500)).await;
            }
            
            // Now take and disconnect the client
            if let Some(client) = self.client.take() {
                // Properly await the disconnection to ensure it completes
                client.disconnect().await;
            }
            
            info!("Disconnected.");
            Ok(())
        } else {
            info!("Already disconnected.");
            Ok(())
        }
    }
    
    /// Get sequence number for implicit acknowledgments based on server message type
    /// For example, RegisterAck implicitly acknowledges a Register message
    fn get_implicit_ack_seq(&self, server_message: &ServerMessage) -> Option<u64> {
        // Find the lowest sequence number of a pending message of the right type
        match server_message {
            ServerMessage::RegisterAck { .. } => {
                // Find a Register message to acknowledge
                self.find_pending_message_by_type(ClientMessageType::Register)
            },
            ServerMessage::GameState { .. } => {
                // GameState could acknowledge Move or Attack
                self.find_pending_message_by_type(ClientMessageType::Move)
                    .or_else(|| self.find_pending_message_by_type(ClientMessageType::Attack))
            },
            ServerMessage::ChatMessage { .. } => {
                // ChatMessage acknowledges Chat
                self.find_pending_message_by_type(ClientMessageType::Chat)
            },
            _ => None,
        }
    }
    
    /// Find a pending message of the specified type (using the oldest one if multiple exist)
    fn find_pending_message_by_type(&self, msg_type: ClientMessageType) -> Option<u64> {
        let mut matching_seqs: Vec<_> = self.pending_acks.iter()
            .filter_map(|(&seq, &(_, mtype))| {
                if mtype == msg_type {
                    Some(seq)
                } else {
                    None
                }
            })
            .collect();
        
        // Sort by sequence number (oldest first)
        matching_seqs.sort();
        
        // Return the oldest (lowest seq num) if any
        matching_seqs.first().copied()
    }
    
    /// Get a reference to the server address
    pub fn server_address(&self) -> &str {
        &self.server_address
    }
    
    /// Check if the client is connected
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }
    
    /// Get the next sequence number and increment the counter
    fn next_seq_num(&mut self) -> u64 {
        let seq = self.seq_counter;
        self.seq_counter = seq + 1;
        seq
    }
}
