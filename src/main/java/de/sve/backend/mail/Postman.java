package de.sve.backend.mail;

import java.util.Arrays;
import java.util.Date;
import java.util.List;
import java.util.Map;
import java.util.Map.Entry;
import java.util.Properties;
import java.util.stream.Collectors;

import jakarta.activation.DataHandler;
import jakarta.mail.Address;
import jakarta.mail.BodyPart;
import jakarta.mail.Message;
import jakarta.mail.MessagingException;
import jakarta.mail.Multipart;
import jakarta.mail.Session;
import jakarta.mail.Transport;
import jakarta.mail.internet.InternetAddress;
import jakarta.mail.internet.MimeBodyPart;
import jakarta.mail.internet.MimeMessage;
import jakarta.mail.internet.MimeMultipart;

/**
 * Resposible for sending emails.
 *
 * @author mseele
 */
public class Postman {

	protected static void checkConnectivity(MailAccount account) throws MessagingException {
		connect(account, (session, transport) -> {
			// do nothing
		});
	}

	protected static void deliver(Mail mail) throws MessagingException {
		deliver(Arrays.asList(mail));
	}

	protected static void deliver(List<Mail> mails) throws MessagingException {
		deliver(mails.stream().collect(Collectors.groupingBy(Mail::sender)));
	}

	private static void deliver(Map<MailAccount, List<Mail>> mails) throws MessagingException {
		for (Entry<MailAccount, List<Mail>> entry : mails.entrySet()) {
			MailAccount account = entry.getKey();
			connect(account, (session, transport) -> {
				// send mails
				for (Mail mail : entry.getValue()) {
					MimeMessage message = new MimeMessage(session);
					// recipients
					message.setFrom(new InternetAddress(account.email()));
					for (String recipient : mail.to()) {
						message.addRecipient(Message.RecipientType.TO, new InternetAddress(recipient));
					}
					for (String recipient : mail.bcc()) {
						message.addRecipient(Message.RecipientType.BCC, new InternetAddress(recipient));
					}
					String replyTo = mail.replyTo();
					if (replyTo != null) {
						message.setReplyTo(new Address[] { new InternetAddress(replyTo) });
					}
					Multipart multipart = new MimeMultipart();
					// subject & send date
					message.setSubject(mail.subject());
					message.setSentDate(new Date());
					// content
					BodyPart content = new MimeBodyPart();
					content.setText(mail.content());
					multipart.addBodyPart(content);
					// attachments
					for (Attachment attachment : mail.attachments()) {
						BodyPart bodyPart = new MimeBodyPart();
						bodyPart.setDataHandler(new DataHandler(attachment.dataSource()));
						bodyPart.setFileName(attachment.name());
						multipart.addBodyPart(bodyPart);
					}
					message.setContent(multipart);
					message.saveChanges();
					// Send message
					transport.sendMessage(message, message.getAllRecipients());
				}
			});
		}
	}

	private static void connect(MailAccount account, IMessageConsumer consumer) throws MessagingException {
		// smtp properties
		Properties properties = System.getProperties();
		String server = "smtp.gmail.com"; //$NON-NLS-1$
		properties.put("mail.smtp.host", server); //$NON-NLS-1$
		properties.put("mail.smtp.port", "465"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.ssl.enable", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.auth", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		// authenticate
		Session session = Session.getInstance(properties);
		try (Transport transport = session.getTransport("smtp")) { //$NON-NLS-1$
			transport.connect(server, account.userName(), account.password());
			consumer.consume( session, transport);
		}
	}

	private static interface IMessageConsumer {

		void consume(Session session, Transport transport) throws MessagingException;

	}

}
