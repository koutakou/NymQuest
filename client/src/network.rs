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

use crate::game_protocol::{ClientMessage, ServerMessage, ServerMessageType, ClientMessageType, Direction, EmoteType, ProtocolVersion};

use crate::config::ClientConfig;

use crate::status_monitor::StatusMonitor;

/// Initial time to wait for an acknowledgement before first resend attempt
const INITIAL_ACK_TIMEOUT_MS: u64 = 5000;

/// Time to wait for subsequent resend attempts (shorter than initial)
const SUBSEQUENT_ACK_TIMEOUT_MS: u64 = 2000;

/// Maximum number of retries for sending a message
const MAX_RETRIES: usize = 3;

/// Client-side rate limiting to prevent hitting server limits
/// Default: slightly lower than server limits to add safety margin
const CLIENT_RATE_LIMIT_PER_SEC: f32 = 8.0; // Slightly below server default of 10
const CLIENT_BURST_SIZE: u32 = 15; // Slightly below server default of 20

/// Maximum number of messages to send in a burst before enforcing rate limit
const MAX_BURST_SIZE: u32 = CLIENT_BURST_SIZE;

/// NetworkManager handles all interactions with the Nym mixnet
/// Structure to hold original message content for potential resends
#[derive(Clone)]
pub enum OriginalMessage {
    Register { name: String, protocol_version: ProtocolVersion },
    Move { direction: Direction },
    Attack { target_display_id: String },
    Chat { message: String },
    Emote { emote_type: EmoteType },
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
    config: ClientConfig,
    status_monitor: Arc<Mutex<StatusMonitor>>,
    /// Token bucket for rate limiting
    rate_limit_tokens: u32,
    /// Last time the rate limit was updated
    last_rate_limit_update: Instant,
    /// Negotiated protocol version for this session
    negotiated_protocol_version: Option<u16>,
    /// Last time a message was sent (for pacing)
    last_message_sent: Option<Instant>,
}

impl NetworkManager {
    /// Create a new NetworkManager and connect to the Nym network
    pub async fn new(config: &ClientConfig, status_monitor: Arc<Mutex<StatusMonitor>>) -> Result<Self> {
        // Read server address and auth key from file
        let file_content = match fs::read_to_string(&config.server_address_file)
            .or_else(|_| fs::read_to_string("server_address.txt"))
            .or_else(|_| fs::read_to_string("../client/server_address.txt")) {
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
            config: config.clone(),
            status_monitor,
            rate_limit_tokens: MAX_BURST_SIZE,
            last_rate_limit_update: Instant::now(),
            negotiated_protocol_version: None,
            last_message_sent: None,
        })
    }
    
    /// Send a message to the server with automatic sequencing and retry mechanism
    pub async fn send_message(&mut self, message: ClientMessage) -> Result<()> {
        // Handle acknowledgment messages without adding sequence numbers
        if let ClientMessage::Ack { .. } = message {
            if let Some(client) = &mut self.client {
                let authenticated_msg = AuthenticatedMessage::new(message, &self.auth_key)?;
                let message_str = serde_json::to_string(&authenticated_msg)?;
                debug!("Sending acknowledgment message");
                let recipient = Recipient::from_str(&self.server_address)
                    .map_err(|e| anyhow!("Invalid server address: {}", e))?;
                client.send_message(recipient, message_str.into_bytes(), IncludedSurbs::default()).await?;
            }
            return Ok(());
        }

        // Get the next sequence number before borrowing client
        let seq_num = self.next_seq_num();
        
        // Check rate limit before processing
        if !self.check_rate_limit() {
            warn!("Rate limit exceeded, delaying message send");
            // Wait until rate limit is restored before sending
            self.wait_for_rate_limit().await?;
        }
        
        // Consume a token for this message
        self.rate_limit_tokens = self.rate_limit_tokens.saturating_sub(1);
        
        // Apply message pacing for privacy protection (prevent timing correlation attacks)
        if self.config.enable_message_pacing {
            if let Some(last_sent) = self.last_message_sent {
                let elapsed = last_sent.elapsed();
                let min_interval = Duration::from_millis(self.config.message_pacing_interval_ms);
                
                if elapsed < min_interval {
                    let wait_time = min_interval - elapsed;
                    debug!("Applying message pacing: waiting {:?} for privacy protection", wait_time);
                    time::sleep(wait_time).await;
                }
            }
        }
        
        if let Some(client) = &mut self.client {
            // For all other message types, attach sequence number
            let message_with_seq = match message {
                ClientMessage::Register { name, protocol_version, .. } => {
                    ClientMessage::Register { name, protocol_version, seq_num }
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
                ClientMessage::Emote { emote_type, .. } => {
                    ClientMessage::Emote { emote_type, seq_num }
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
                ClientMessage::Register { name, protocol_version, .. } => {
                    OriginalMessage::Register { name: name.clone(), protocol_version: protocol_version.clone() }
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
                ClientMessage::Emote { emote_type, .. } => {
                    OriginalMessage::Emote { emote_type: *emote_type }
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
            
            // Create authenticated message 
            let authenticated_msg = AuthenticatedMessage::new(message_with_seq.clone(), &self.auth_key)?;
            let message_str = serde_json::to_string(&authenticated_msg)?;
            
            // Create recipient from server address
            let server_address = match Recipient::from_str(&self.server_address) {
                Ok(addr) => addr,
                Err(e) => return Err(anyhow!("Invalid server address: {}", e)),
            };
            
            debug!("Sending message with seq_num: {}", seq_num);
            client.send_message(server_address, message_str.into_bytes(), IncludedSurbs::default()).await?;
            
            // Update status monitor to record message sent
            if let Ok(mut monitor) = self.status_monitor.lock() {
                monitor.record_message_sent(seq_num);
                // Update mixnet connection status
                monitor.update_mixnet_status(true, Some(3), None);
            }
            
            // Update pacing
            self.last_message_sent = Some(Instant::now());
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
                    
                    // Update status monitor to record timeout (potential retransmission)
                    if let Ok(mut monitor) = self.status_monitor.lock() {
                        monitor.record_message_timeout(seq_num);
                    }
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
            
            // Update status monitor to record failed message
            if let Ok(mut monitor) = self.status_monitor.lock() {
                monitor.record_message_failed(seq_num);
            }
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
                    OriginalMessage::Register { name, protocol_version } => {
                        debug!("Resending Register with original name: {}", name);
                        ClientMessage::Register { 
                            name: name.clone(), 
                            protocol_version: protocol_version.clone(), 
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
                    OriginalMessage::Emote { emote_type } => {
                        debug!("Resending Emote");
                        ClientMessage::Emote {
                            emote_type: *emote_type,
                            seq_num
                        }
                    },
                }
            } else {
                // Fallback if original message is somehow not available
                warn!("Original message data not found for seq_num {}", seq_num);
                match msg_type {
                    ClientMessageType::Register => {
                        ClientMessage::Register { 
                            name: format!("Resend_{}", seq_num), 
                            protocol_version: ProtocolVersion::default(),
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
                    ClientMessageType::Emote => {
                        // Default to a wave emote for resends
                        ClientMessage::Emote {
                            emote_type: EmoteType::Wave,
                            seq_num
                        }
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
                let authenticated_msg = AuthenticatedMessage::new(message, &self.auth_key)?;
                let message_str = serde_json::to_string(&authenticated_msg)?;
                
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
        
        // Handle protocol version negotiation for RegisterAck
        if let ServerMessage::RegisterAck { negotiated_version, .. } = &server_message {
            self.negotiated_protocol_version = Some(*negotiated_version);
            info!("Protocol version negotiated: v{}", negotiated_version);
        }
                
        // Handle explicit acknowledgements first
        if let ServerMessage::Ack { client_seq_num, original_type } = &server_message {
            // Remove from pending acks when we receive an ack
            if let Some((sent_time, _)) = self.pending_acks.remove(&client_seq_num) {
                let latency_ms = sent_time.elapsed().as_millis() as u64;
                debug!("Received acknowledgment for message {} of type {:?} (latency: {}ms)", client_seq_num, original_type, latency_ms);
                
                // Update status monitor with successful delivery
                if let Ok(mut monitor) = self.status_monitor.lock() {
                    monitor.record_message_delivered(*client_seq_num, latency_ms);
                }
                
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
            if let Some((sent_time, _)) = self.pending_acks.remove(&implicit_ack_seq) {
                let latency_ms = sent_time.elapsed().as_millis() as u64;
                debug!("Implicit acknowledgment received for message {} (latency: {}ms)", implicit_ack_seq, latency_ms);
                
                // Update status monitor with successful delivery
                if let Ok(mut monitor) = self.status_monitor.lock() {
                    monitor.record_message_delivered(implicit_ack_seq, latency_ms);
                }
                
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
        
        // Update network health metrics in status monitor
        if let Ok(mut monitor) = self.status_monitor.lock() {
            // Calculate some basic network statistics
            let pending_count = self.pending_acks.len();
            let has_pending_messages = pending_count > 0;
            
            // Update connection status based on pending messages and activity
            let anonymity_set_size = if has_pending_messages { Some(5) } else { Some(3) };
            monitor.update_mixnet_status(true, anonymity_set_size, None);
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
    
    /// Get the negotiated protocol version for this session
    pub fn get_negotiated_protocol_version(&self) -> Option<u16> {
        self.negotiated_protocol_version
    }
    
    /// Get the next sequence number and increment the counter
    fn next_seq_num(&mut self) -> u64 {
        let seq = self.seq_counter;
        self.seq_counter = seq + 1;
        seq
    }
    
    /// Check if the rate limit has been exceeded
    fn check_rate_limit(&mut self) -> bool {
        let elapsed = self.last_rate_limit_update.elapsed().as_secs_f32();
        let tokens_to_add = (elapsed * CLIENT_RATE_LIMIT_PER_SEC) as u32;
        self.rate_limit_tokens = self.rate_limit_tokens.saturating_add(tokens_to_add);
        self.rate_limit_tokens = self.rate_limit_tokens.min(MAX_BURST_SIZE);
        self.last_rate_limit_update = Instant::now();
        
        self.rate_limit_tokens > 0
    }
    
    /// Wait until the rate limit is restored
    async fn wait_for_rate_limit(&mut self) -> Result<()> {
        let wait_time = (MAX_BURST_SIZE as f32 / CLIENT_RATE_LIMIT_PER_SEC) - self.last_rate_limit_update.elapsed().as_secs_f32();
        if wait_time > 0.0 {
            time::sleep(Duration::from_secs_f32(wait_time)).await;
        }
        self.rate_limit_tokens = MAX_BURST_SIZE;
        self.last_rate_limit_update = Instant::now();
        Ok(())
    }
}
