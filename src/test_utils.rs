use std::sync::{Arc, Mutex};

use lettre::Message;

use crate::email::MockEmailSender;
use crate::models::{EmailAccount, EmailType};

/// No-op mock — no email methods are called.
/// For tests where email sending is never reached (e.g. draft events, invalid input).
pub(crate) fn noop_mock() -> MockEmailSender {
    MockEmailSender::new()
}

/// Mock that resolves email accounts and discards sent messages.
/// Panics if any unconfigured email type is requested.
pub(crate) fn mock_email_sender(accounts: Vec<(EmailType, &str)>) -> MockEmailSender {
    let mut mock = MockEmailSender::new();

    for (email_type, address) in accounts {
        let account = EmailAccount::new_for_test(email_type.clone(), address);
        mock.expect_get_account_by_type()
            .withf(move |t| t == &email_type)
            .times(1)
            .returning(move |_| {
                let account = account.clone();
                Box::pin(async move { Ok(account) })
            });
    }

    mock.expect_send_message()
        .returning(|_, _| Box::pin(async { Ok(()) }));
    mock.expect_send_messages()
        .returning(|_, _| Box::pin(async { Ok(()) }));

    mock
}

/// Mock that resolves accounts and captures all sent messages.
/// Returns (mock, captured_messages).
pub(crate) fn mock_email_sender_capturing(
    accounts: Vec<(EmailType, &str)>,
) -> (MockEmailSender, Arc<Mutex<Vec<Message>>>) {
    let mut mock = MockEmailSender::new();

    for (email_type, address) in accounts {
        let account = EmailAccount::new_for_test(email_type.clone(), address);
        mock.expect_get_account_by_type()
            .withf(move |t| t == &email_type)
            .times(1)
            .returning(move |_| {
                let account = account.clone();
                Box::pin(async move { Ok(account) })
            });
    }

    let captured = Arc::new(Mutex::new(Vec::new()));
    let for_return = captured.clone();
    let for_send_message = captured.clone();
    let for_send_messages = captured.clone();
    mock.expect_send_message().returning(move |_, message| {
        for_send_message.lock().unwrap().push(message);
        Box::pin(async { Ok(()) })
    });
    mock.expect_send_messages().returning(move |_, messages| {
        for_send_messages.lock().unwrap().extend(messages);
        Box::pin(async { Ok(()) })
    });

    (mock, for_return)
}

/// Mock that resolves accounts and captures (account, messages) per send_messages call.
/// Returns (mock, captured_batches).
pub(crate) fn mock_email_sender_capturing_batch(
    accounts: Vec<(EmailType, &str)>,
) -> (
    MockEmailSender,
    Arc<Mutex<Vec<(EmailAccount, Vec<Message>)>>>,
) {
    let mut mock = MockEmailSender::new();

    for (email_type, address) in accounts {
        let account = EmailAccount::new_for_test(email_type.clone(), address);
        mock.expect_get_account_by_type()
            .withf(move |t| t == &email_type)
            .times(1)
            .returning(move |_| {
                let account = account.clone();
                Box::pin(async move { Ok(account) })
            });
    }

    let captured = Arc::new(Mutex::new(Vec::new()));
    let for_return = captured.clone();
    let for_send_messages = captured.clone();
    let for_send_message = captured.clone();
    mock.expect_send_messages()
        .returning(move |account, messages| {
            for_send_messages
                .lock()
                .unwrap()
                .push((account.clone(), messages));
            Box::pin(async { Ok(()) })
        });
    mock.expect_send_message()
        .returning(move |account, message| {
            for_send_message
                .lock()
                .unwrap()
                .push((account.clone(), vec![message]));
            Box::pin(async { Ok(()) })
        });

    (mock, for_return)
}
