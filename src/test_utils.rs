use std::sync::{Arc, Mutex};

use lettre::Message;

use crate::email::MockEmailGateway;
use crate::models::{EmailAccount, EmailType};

/// Canonical mock constructor for the email seam.
///
/// Configures the gateway to resolve the supplied accounts by type and captures
/// every `send_messages` call as `(account, messages)` batches.
pub(crate) fn mock_email_gateway(
    accounts: Vec<(EmailType, &str)>,
) -> (
    MockEmailGateway,
    Arc<Mutex<Vec<(EmailAccount, Vec<Message>)>>>,
) {
    let mut mock = MockEmailGateway::new();

    for (email_type, address) in accounts {
        let account = EmailAccount::new_for_test(email_type.clone(), address);
        mock.expect_account_by_type()
            .withf(move |t| t == &email_type)
            .times(..)
            .returning(move |_| {
                let account = account.clone();
                Box::pin(async move { Ok(account) })
            });
    }

    mock.expect_build_message()
        .returning(|account| Ok(Message::builder().from(account.address.parse()?).date_now()));

    let captured = Arc::new(Mutex::new(Vec::new()));
    let for_return = captured.clone();
    let for_send_messages = captured.clone();
    mock.expect_send_messages()
        .returning(move |account, messages| {
            for_send_messages
                .lock()
                .unwrap()
                .push((account.clone(), messages));
            Box::pin(async { Ok(()) })
        });

    (mock, for_return)
}
